// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! OpenHarmony backend for the global-shortcut plugin.
//!
//! OHOS has no `global-hotkey` crate: shortcut registration goes through the
//! openharmony-ability bridge (`GlobalShortcutClient` facade), which dispatches
//! to the ArkTS `inputConsumer` API on the app main thread. This module holds:
//! - drop-in replacements for the `global_hotkey` types re-exported by the
//!   crate root on desktop (`Shortcut`, `Modifiers`, `Code`, `ShortcutEvent`,
//!   `ShortcutState`)
//! - the fire-and-forget register/unregister worker helpers
//! - the plugin setup (`ohos_setup`) invoked by `Builder::build`

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use openharmony_ability_plugin_global_shortcut::{
    GlobalShortcutBridgePlugin, GlobalShortcutClient, GlobalShortcutExt,
};

use crate::{GlobalShortcut, HandlerFn, HotKeyId, RegisteredShortcut};

// ─── Shortcut types (drop-in for the global_hotkey re-exports) ──────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShortcutState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutEvent {
    pub id: u32,
    pub state: ShortcutState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifiers {
    CONTROL,
    ALT,
    SHIFT,
    SUPER,
}

impl Modifiers {
    pub fn to_ohos_name(&self) -> &'static str {
        match self {
            Modifiers::CONTROL => "Control",
            Modifiers::ALT => "Alt",
            Modifiers::SHIFT => "Shift",
            Modifiers::SUPER => "Super",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            // Note: "cmd"/"command" maps to CONTROL on OHOS (no macOS ⌘ key).
            // On desktop these map to SUPER.
            "ctrl" | "control" | "commandorcontrol" | "cmdorctrl"
                | "commandorctrl" | "cmdorcontrol" | "command" | "cmd" => {
                Some(Modifiers::CONTROL)
            }
            "alt" | "option" => Some(Modifiers::ALT),
            "shift" => Some(Modifiers::SHIFT),
            "super" | "meta" | "win" | "windows" => Some(Modifiers::SUPER),
            _ => None,
        }
    }
}

/// Minimal Code enum — only stores the key name string.
/// The actual key code mapping happens in the ArkTS layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Code(String);

impl Code {
    pub fn to_ohos_name(&self) -> &str {
        &self.0
    }

    pub fn from_name(name: &str) -> Option<Self> {
        // Accept any key name; validation happens in ArkTS.
        // Normalize desktop `global-hotkey` Code variant names so that
        // shortcuts authored as "Ctrl+KeyA" / "Ctrl+Digit1" / "Ctrl+a"
        // (valid on desktop) resolve to the OHOS-accepted "A" / "1" / "A".
        if name.is_empty() {
            return None;
        }
        let normalized = normalize_key_name(name);
        Some(Code(normalized))
    }
}

/// Normalize a desktop `global-hotkey` key name to the OHOS-accepted form.
///
/// Desktop accepts `KeyA`, `Digit1`, `a`, `A` interchangeably; OHOS only
/// matches the canonical `A`, `1`, `F5`, `Space`, ... form. This strips
/// the `Key`/`Digit` prefixes and uppercases single letters so a shortcut
/// authored on desktop keeps working on OHOS.
fn normalize_key_name(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("Key") {
        // KeyA -> A
        rest.to_string()
    } else if let Some(rest) = name.strip_prefix("Digit") {
        // Digit1 -> 1
        rest.to_string()
    } else if name.len() == 1 {
        // a -> A (single char keys are case-insensitive on desktop)
        name.to_uppercase()
    } else {
        // F1, Space, Enter, ... already canonical
        name.to_string()
    }
}

/// OHOS-compatible Shortcut type.
/// Parses the same string format as global-hotkey: "CmdOrCtrl+Shift+A"
#[derive(Debug, Clone)]
pub struct Shortcut {
    id: u32,
    modifiers: Vec<Modifiers>,
    code: Code,
    original: String,
}

impl Shortcut {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn modifiers(&self) -> &[Modifiers] {
        &self.modifiers
    }

    pub fn code(&self) -> &Code {
        &self.code
    }

    pub fn into_string(&self) -> String {
        self.original.clone()
    }
}

impl std::str::FromStr for Shortcut {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            return Err("Empty shortcut string".to_string());
        }

        let key_name = parts.last().ok_or_else(|| "Empty shortcut string".to_string())?;
        let code = Code::from_name(key_name)
            .ok_or_else(|| format!("Unknown key: {}", key_name))?;

        // All tokens except the last must be valid modifiers
        let mut modifiers = Vec::new();
        for &part in &parts[..parts.len() - 1] {
            let lower = part.to_lowercase();
            let m = Modifiers::from_name(&lower)
                .ok_or_else(|| format!("Invalid modifier: {}", part))?;
            // Deduplicate modifiers
            if !modifiers.contains(&m) {
                modifiers.push(m);
            }
        }

        // OHOS inputConsumer.preKeys constraint: 1-2 modifier keys required.
        // Rejecting at parse time ensures register() returns Err before the
        // shortcut enters the HashMap, so isRegistered returns false.
        if modifiers.is_empty() {
            return Err("At least 1 modifier key is required".to_string());
        }
        if modifiers.len() > 2 {
            return Err(format!(
                "OHOS supports at most 2 modifier keys, got {}",
                modifiers.len()
            ));
        }

        // Deterministic ID based on shortcut content (matches global-hotkey behavior)
        let normalized = s.to_lowercase();
        // Deterministic hash-based ID (djb2 variant). Collision risk is negligible
        // for typical shortcut strings (e.g. "ctrl+shift+t") but theoretically possible
        // for carefully crafted inputs.
        let id = normalized.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));

        Ok(Shortcut {
            id,
            modifiers,
            code,
            original: s.to_string(),
        })
    }
}

impl From<&Shortcut> for String {
    fn from(s: &Shortcut) -> String {
        s.original.clone()
    }
}

/// Error type for shortcut parsing (matches global-hotkey's HotKeyParseError).
#[derive(Debug)]
pub struct HotKeyParseError(pub String);

impl std::fmt::Display for HotKeyParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse hotkey: {}", self.0)
    }
}

impl std::error::Error for HotKeyParseError {}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Convert OHOS stub modifiers to facade-compatible string names.
/// Returns `"Control"`, `"Alt"`, `"Shift"`, `"Super"` — matching the
/// `GlobalShortcutClient::register()` modifier name contract.
fn to_ohos_modifier_names(modifiers: &[Modifiers]) -> Vec<String> {
    modifiers
        .iter()
        .map(|m| m.to_ohos_name().to_string())
        .collect()
}

/// Locks the shortcuts store. OHOS recovers from a poisoned lock (a panicking
/// bridge thread must not permanently break shortcut registration); other
/// platforms keep the upstream panicking `unwrap` semantics.
pub(crate) fn lock_shortcuts<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Runs `f` against the global OHOS app instance held in [`tauri::ohos::APP`].
/// Returns `None` when the app is not initialized yet or its lock is poisoned.
fn with_ohos_app<R>(
    f: impl FnOnce(&tauri::ohos::openharmony_ability::OpenHarmonyApp) -> R,
) -> Option<R> {
    let guard = tauri::ohos::APP.lock().ok()?;
    guard.as_ref().map(f)
}

// ─── Fire-and-forget register/unregister ────────────────────────────────────

/// Monotonic stamp source for registration generations; see
/// `RegisteredShortcut::stamp` in lib.rs.
pub(crate) fn next_registration_stamp() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Register a shortcut on a worker thread (fire-and-forget).
///
/// The facade's async `register()` dispatches through the bridge TSFN to the
/// ArkTS main thread; blocking on the main thread would deadlock, so each
/// call spawns a worker thread and `block_on`s there. Callers insert into the
/// store synchronously before spawning (a hotkey that fires immediately must
/// find its handler); when ArkTS later rejects the registration, the worker
/// removes that entry again so `isRegistered` cannot report a ghost
/// registration — but only when the entry still carries this worker's stamp:
/// if the same id was re-registered in between (replacing the entry with a
/// newer stamp), the stale worker must not undo the newer registration.
pub(crate) fn spawn_register<R: Runtime>(
    client: GlobalShortcutClient,
    id: HotKeyId,
    modifiers: &[Modifiers],
    key: &str,
    stamp: u64,
    shortcuts: Arc<Mutex<HashMap<HotKeyId, RegisteredShortcut<R>>>>,
) {
    let modifier_names = to_ohos_modifier_names(modifiers);
    let key = key.to_string();
    std::thread::spawn(move || {
        log::info!("[global-shortcut] register ENTER id={}", id);
        if let Err(e) = futures_executor::block_on(client.register(id, &modifier_names, &key)) {
            log::error!("[global-shortcut] Failed to register shortcut {}: {:?}", id, e);
            let mut shortcuts = lock_shortcuts(&shortcuts);
            if shortcuts.get(&id).is_some_and(|entry| entry.stamp == stamp) {
                shortcuts.remove(&id);
            }
        }
    });
}

/// Unregister a shortcut on a worker thread (fire-and-forget); failures are
/// log-only by design.
pub(crate) fn spawn_unregister(client: GlobalShortcutClient, id: HotKeyId) {
    std::thread::spawn(move || {
        if let Err(e) = futures_executor::block_on(client.unregister(id)) {
            log::error!("[global-shortcut] Failed to unregister shortcut {}: {:?}", id, e);
        }
    });
}

/// Unregister all shortcuts on a worker thread (fire-and-forget); failures
/// are log-only by design.
pub(crate) fn spawn_unregister_all(client: GlobalShortcutClient) {
    std::thread::spawn(move || {
        if let Err(e) = futures_executor::block_on(client.unregister_all()) {
            log::error!("[global-shortcut] Failed to unregister all shortcuts: {:?}", e);
        }
    });
}

// ─── Plugin setup ───────────────────────────────────────────────────────────

/// OHOS backend setup: create the GlobalShortcutClient facade, register
/// all shortcuts, spawn the event-receiver thread, and manage the
/// `GlobalShortcut` state. Extracted from `build()`'s setup closure so the
/// shared closure body stays a one-line cfg dispatch (see reference §1.6).
pub(crate) fn ohos_setup<R: Runtime>(
    app: AppHandle<R>,
    shortcuts: Vec<Shortcut>,
    handler: Option<HandlerFn<R>>,
    store: HashMap<HotKeyId, RegisteredShortcut<R>>,
) {
    // Register the Rust-side GlobalShortcut bridge plugin so ArkTS configurePlugins
    // can match it. Without this, bridge calls fail with
    // "Bridge plugin 'ohos.global-shortcut' is not installed for '<module>'".
    let (app_ready, bridge_plugin_registered) = match with_ohos_app(|ohos_app| {
        match ohos_app.register_plugin(GlobalShortcutBridgePlugin) {
            Ok(()) => true,
            Err(e) => {
                log::error!(
                    "[global-shortcut] failed to register GlobalShortcutBridgePlugin: {}",
                    e
                );
                false
            }
        }
    }) {
        Some(registered) => (true, registered),
        None => (false, false),
    };

    // Obtain GlobalShortcutClient from the global OHOS app
    let (client, client_err) = match with_ohos_app(|app| app.global_shortcut()) {
        Some(Ok(c)) => (Some(c), None),
        Some(Err(e)) => (
            None,
            Some(format!("global_shortcut() returned Err: {:?}", e)),
        ),
        None => (
            None,
            Some("tauri::ohos::APP not ready (guard None)".to_string()),
        ),
    };

    // Diagnostic: capture the three states that determine whether shortcut
    // registration can reach the OS at ohos_setup time. This is the key log
    // for diagnosing the "Ctrl+Shift+T no response" issue — if client is
    // None here, every fire-and-forget register is silently skipped.
    log::info!(
        "[global-shortcut] ohos_setup: app_ready={}, bridge_plugin_registered={}, client={}, shortcut_count={}",
        app_ready,
        bridge_plugin_registered,
        client.is_some(),
        shortcuts.len()
    );
    if let Some(ref err) = client_err {
        log::info!(
            "[global-shortcut] ohos_setup: client is None because: {}",
            err
        );
    }

    // Wrap the store first: entries are inserted synchronously (a hotkey that
    // fires immediately must find its handler) and the register workers get
    // the shared handle so an ArkTS rejection removes the entry again
    // (no ghost registrations in `isRegistered`). Each entry is stamped and
    // dispatched with the same stamp so the removal stays generation-aware.
    let shortcuts_store = Arc::new(Mutex::new(store));
    for shortcut in &shortcuts {
        // Stamp under the same lock as the insert (see register_internal in
        // lib.rs) so the stamp order always matches the insert order.
        let stamp = {
            let mut store = lock_shortcuts(&shortcuts_store);
            let stamp = next_registration_stamp();
            store.insert(
                shortcut.id(),
                RegisteredShortcut {
                    shortcut: shortcut.clone(),
                    handler: None,
                    stamp,
                },
            );
            stamp
        };
        if let Some(ref client) = client {
            spawn_register(
                client.clone(),
                shortcut.id(),
                shortcut.modifiers(),
                shortcut.code().to_ohos_name(),
                stamp,
                shortcuts_store.clone(),
            );
        }
    }

    if client.is_none() {
        log::warn!(
            "GlobalShortcutClient not initialized; skipping shortcut registration"
        );
    }

    let shortcuts = shortcuts_store;
    let shortcuts_ = shortcuts.clone();
    let app_handle = app.clone();

    // Spawn thread to receive shortcut events from the facade.
    // The event_receiver() returns a &'static Receiver, so it's safe to use
    // in a 'static thread closure. This thread runs for the entire app lifetime.
    if let Some(ref client) = client {
        let receiver = client.event_receiver();
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                // Clone needed data and drop the lock before calling user callbacks
                // to avoid deadlock if the callback tries to acquire the same lock.
                let entry = lock_shortcuts(&shortcuts_)
                    .get(&event.id)
                    .map(|reg| (reg.handler.clone(), reg.shortcut.clone()));

                if let Some((handler_opt, shortcut)) = entry {
                    let shortcut_event = ShortcutEvent {
                        id: event.id,
                        state: match event.state.as_str() {
                            "Pressed" => ShortcutState::Pressed,
                            "Released" => ShortcutState::Released,
                            _ => ShortcutState::Pressed,
                        },
                    };

                    if let Some(h) = &handler_opt {
                        h(&app_handle, &shortcut, shortcut_event.clone());
                    }
                    if let Some(h) = &handler {
                        h(&app_handle, &shortcut, shortcut_event);
                    }
                }
            }
        });
    }

    app.manage(GlobalShortcut { shortcuts, client });
}
