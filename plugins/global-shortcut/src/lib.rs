// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Register global shortcuts.
//!
//! - Supported platforms: Windows, Linux, macOS and OpenHarmony.

#![doc(
    html_logo_url = "https://github.com/tauri-apps/tauri/raw/dev/app-icon.png",
    html_favicon_url = "https://github.com/tauri-apps/tauri/raw/dev/app-icon.png"
)]
#![cfg(not(any(target_os = "android", target_os = "ios")))]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tauri::{
    ipc::Channel,
    plugin::{Builder as PluginBuilder, TauriPlugin},
    AppHandle, Manager, Runtime, State,
};

mod error;

pub use error::Error;
type Result<T> = std::result::Result<T, Error>;

type HotKeyId = u32;

// ─── Platform-specific type aliases ─────────────────────────────────────────

#[cfg(not(target_env = "ohos"))]
pub use global_hotkey::{
    hotkey::{Code, HotKey as Shortcut, Modifiers},
    GlobalHotKeyEvent as ShortcutEvent, HotKeyState as ShortcutState,
};

#[cfg(not(target_env = "ohos"))]
use global_hotkey::GlobalHotKeyEvent;

#[cfg(not(target_env = "ohos"))]
use std::str::FromStr;

// ─── OHOS stub types ────────────────────────────────────────────────────────

#[cfg(target_env = "ohos")]
mod ohos_types {
    use serde::{Deserialize, Serialize};

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
}

#[cfg(target_env = "ohos")]
pub use ohos_types::{Code, Modifiers, Shortcut, ShortcutEvent, ShortcutState};

// ─── Common types ───────────────────────────────────────────────────────────

type HandlerFn<R> = Box<dyn Fn(&AppHandle<R>, &Shortcut, ShortcutEvent) + Send + Sync + 'static>;

pub struct ShortcutWrapper(Shortcut);

impl From<Shortcut> for ShortcutWrapper {
    fn from(value: Shortcut) -> Self {
        Self(value)
    }
}

#[cfg(not(target_env = "ohos"))]
impl TryFrom<&str> for ShortcutWrapper {
    type Error = global_hotkey::hotkey::HotKeyParseError;
    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        Shortcut::from_str(value).map(ShortcutWrapper)
    }
}

#[cfg(target_env = "ohos")]
impl TryFrom<&str> for ShortcutWrapper {
    type Error = ohos_types::HotKeyParseError;
    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        value
            .parse::<Shortcut>()
            .map(ShortcutWrapper)
            .map_err(|e| ohos_types::HotKeyParseError(e))
    }
}

struct RegisteredShortcut<R: Runtime> {
    shortcut: Shortcut,
    handler: Option<Arc<HandlerFn<R>>>,
}

// ─── Platform-specific GlobalHotKeyManager ──────────────────────────────────

#[cfg(not(target_env = "ohos"))]
struct GlobalHotKeyManager(global_hotkey::GlobalHotKeyManager);

#[cfg(not(target_env = "ohos"))]
/// SAFETY: we ensure it is run on main thread only
unsafe impl Send for GlobalHotKeyManager {}
#[cfg(not(target_env = "ohos"))]
/// SAFETY: we ensure it is run on main thread only
unsafe impl Sync for GlobalHotKeyManager {}

pub struct GlobalShortcut<R: Runtime> {
    #[cfg(not(target_env = "ohos"))]
    app: AppHandle<R>,
    #[cfg(not(target_env = "ohos"))]
    manager: Arc<GlobalHotKeyManager>,
    #[cfg(target_env = "ohos")]
    client: Option<openharmony_ability_plugin_global_shortcut::GlobalShortcutClient>,
    shortcuts: Arc<Mutex<HashMap<HotKeyId, RegisteredShortcut<R>>>>,
}

#[cfg(not(target_env = "ohos"))]
macro_rules! run_main_thread {
    ($handle:expr, $manager:expr, |$m:ident| $ex:expr) => {{
        let (tx, rx) = std::sync::mpsc::channel();
        let manager = $manager.clone();
        let task = move || {
            let f = |$m: &GlobalHotKeyManager| $ex;
            let _ = tx.send(f(&*manager));
        };
        $handle.run_on_main_thread(task)?;
        rx.recv()?
    }};
}

/// Convert OHOS stub modifiers to facade-compatible string names.
/// Returns `"Control"`, `"Alt"`, `"Shift"`, `"Super"` — matching the
/// `GlobalShortcutClient::register()` modifier name contract.
#[cfg(target_env = "ohos")]
fn to_ohos_modifier_names(modifiers: &[Modifiers]) -> Vec<String> {
    modifiers
        .iter()
        .map(|m| m.to_ohos_name().to_string())
        .collect()
}

/// OHOS backend setup: create the GlobalShortcutClient facade, register
/// all shortcuts, spawn the event-receiver thread, and manage the
/// `GlobalShortcut` state. Extracted from `build()`'s setup closure so the
/// shared closure body stays a one-line cfg dispatch (see reference §1.6).
#[cfg(target_env = "ohos")]
fn ohos_setup<R: Runtime>(
    app: AppHandle<R>,
    shortcuts: Vec<Shortcut>,
    handler: Option<HandlerFn<R>>,
    mut store: HashMap<HotKeyId, RegisteredShortcut<R>>,
) {
    use openharmony_ability_plugin_global_shortcut::{GlobalShortcutBridgePlugin, GlobalShortcutExt};

    // Register the Rust-side GlobalShortcut bridge plugin so ArkTS configurePlugins
    // can match it. Without this, bridge calls fail with
    // "Bridge plugin 'ohos.global-shortcut' is not installed for '<module>'".
    let (app_ready, bridge_plugin_registered) = {
        let guard = tauri::ohos::APP.lock().ok();
        let app_ref = guard.as_ref().and_then(|g| g.as_ref());
        let app_ready = app_ref.is_some();
        let bridge_plugin_registered = if let Some(ohos_app) = app_ref {
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
        } else {
            false
        };
        (app_ready, bridge_plugin_registered)
    };

    // Obtain GlobalShortcutClient from the global OHOS app
    let (client, client_err) = {
        let guard = tauri::ohos::APP.lock().ok();
        let app_ref = guard.as_ref().and_then(|g| g.as_ref());
        match app_ref {
            Some(app) => match app.global_shortcut() {
                Ok(c) => (Some(c), None),
                Err(e) => (
                    None,
                    Some(format!("global_shortcut() returned Err: {:?}", e)),
                ),
            },
            None => (
                None,
                Some("tauri::ohos::APP not ready (guard None)".to_string()),
            ),
        }
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

    // Register all shortcuts via fire-and-forget worker threads.
    // The facade's async register() dispatches through the bridge TSFN to
    // the ArkTS main thread; blocking on the main thread would deadlock,
    // so we spawn a worker thread and block_on there.
    if let Some(ref client) = client {
        for shortcut in &shortcuts {
            let modifier_names = to_ohos_modifier_names(shortcut.modifiers());
            let key = shortcut.code().to_ohos_name().to_string();
            let id = shortcut.id();
            let client = client.clone();
            std::thread::spawn(move || {
                if let Err(e) = futures_executor::block_on(
                    client.register(id, &modifier_names, &key),
                ) {
                    log::error!("[global-shortcut] Failed to register shortcut {}: {:?}", id, e);
                }
            });
        }
    } else {
        log::warn!(
            "GlobalShortcutClient not initialized; skipping shortcut registration"
        );
    }

    // Insert all shortcuts into the store regardless of registration result
    for shortcut in shortcuts {
        store.insert(
            shortcut.id(),
            RegisteredShortcut {
                shortcut,
                handler: None,
            },
        );
    }

    let shortcuts = Arc::new(Mutex::new(store));
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
                let entry = shortcuts_
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
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

impl<R: Runtime> GlobalShortcut<R> {
    fn register_internal<F: Fn(&AppHandle<R>, &Shortcut, ShortcutEvent) + Send + Sync + 'static>(
        &self,
        shortcut: Shortcut,
        handler: Option<F>,
    ) -> Result<()> {
        let id = shortcut.id();
        let handler = handler.map(|h| Arc::new(Box::new(h) as HandlerFn<R>));

        #[cfg(not(target_env = "ohos"))]
        {
            run_main_thread!(self.app, self.manager, |m| m.0.register(shortcut.clone()))?;
        }

        #[cfg(target_env = "ohos")]
        {
            let client = self.client.as_ref().ok_or_else(|| {
                Error::GlobalHotkey("GlobalShortcutClient not initialized".to_string())
            })?;
            let modifier_names = to_ohos_modifier_names(shortcut.modifiers());
            let key = shortcut.code().to_ohos_name().to_string();
            let client = client.clone();
            std::thread::spawn(move || {
                log::info!("[global-shortcut] register ENTER id={}", id);
                if let Err(e) =
                    futures_executor::block_on(client.register(id, &modifier_names, &key))
                {
                    log::error!("[global-shortcut] Failed to register shortcut {}: {:?}", id, e);
                }
            });
        }

        self.shortcuts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, RegisteredShortcut { shortcut, handler });
        Ok(())
    }

    fn register_multiple_internal<S, F>(&self, shortcuts: S, handler: Option<F>) -> Result<()>
    where
        S: IntoIterator<Item = Shortcut>,
        F: Fn(&AppHandle<R>, &Shortcut, ShortcutEvent) + Send + Sync + 'static,
    {
        let handler = handler.map(|h| Arc::new(Box::new(h) as HandlerFn<R>));

        let hotkeys = shortcuts.into_iter().collect::<Vec<_>>();

        let mut shortcuts = self.shortcuts.lock().unwrap_or_else(|e| e.into_inner());
        for shortcut in hotkeys {
            #[cfg(not(target_env = "ohos"))]
            {
                run_main_thread!(self.app, self.manager, |m| m.0.register(shortcut.clone()))?;
            }

            #[cfg(target_env = "ohos")]
            {
                if let Some(ref client) = self.client {
                    let modifier_names = to_ohos_modifier_names(shortcut.modifiers());
                    let key = shortcut.code().to_ohos_name().to_string();
                    let sid = shortcut.id();
                    let client = client.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = futures_executor::block_on(
                            client.register(sid, &modifier_names, &key),
                        ) {
                            log::error!("[global-shortcut] Failed to register shortcut {}: {:?}", sid, e);
                        }
                    });
                }
            }

            shortcuts.insert(
                shortcut.id(),
                RegisteredShortcut {
                    shortcut,
                    handler: handler.clone(),
                },
            );
        }

        Ok(())
    }
}

impl<R: Runtime> GlobalShortcut<R> {
    /// Register a shortcut.
    pub fn register<S>(&self, shortcut: S) -> Result<()>
    where
        S: TryInto<ShortcutWrapper>,
        S::Error: std::error::Error,
    {
        self.register_internal(
            try_into_shortcut(shortcut)?,
            None::<fn(&AppHandle<R>, &Shortcut, ShortcutEvent)>,
        )
    }

    /// Register a shortcut with a handler.
    pub fn on_shortcut<S, F>(&self, shortcut: S, handler: F) -> Result<()>
    where
        S: TryInto<ShortcutWrapper>,
        S::Error: std::error::Error,
        F: Fn(&AppHandle<R>, &Shortcut, ShortcutEvent) + Send + Sync + 'static,
    {
        self.register_internal(try_into_shortcut(shortcut)?, Some(handler))
    }

    /// Register multiple shortcuts.
    pub fn register_multiple<S, T>(&self, shortcuts: S) -> Result<()>
    where
        S: IntoIterator<Item = T>,
        T: TryInto<ShortcutWrapper>,
        T::Error: std::error::Error,
    {
        let mut s = Vec::new();
        for shortcut in shortcuts {
            s.push(try_into_shortcut(shortcut)?);
        }
        self.register_multiple_internal(s, None::<fn(&AppHandle<R>, &Shortcut, ShortcutEvent)>)
    }

    /// Register multiple shortcuts with a handler.
    pub fn on_shortcuts<S, T, F>(&self, shortcuts: S, handler: F) -> Result<()>
    where
        S: IntoIterator<Item = T>,
        T: TryInto<ShortcutWrapper>,
        T::Error: std::error::Error,
        F: Fn(&AppHandle<R>, &Shortcut, ShortcutEvent) + Send + Sync + 'static,
    {
        let mut s = Vec::new();
        for shortcut in shortcuts {
            s.push(try_into_shortcut(shortcut)?);
        }
        self.register_multiple_internal(s, Some(handler))
    }

    /// Unregister a shortcut
    pub fn unregister<S: TryInto<ShortcutWrapper>>(&self, shortcut: S) -> Result<()>
    where
        S::Error: std::error::Error,
    {
        let shortcut = try_into_shortcut(shortcut)?;

        #[cfg(not(target_env = "ohos"))]
        {
            run_main_thread!(self.app, self.manager, |m| m.0.unregister(shortcut.clone()))?;
        }

        #[cfg(target_env = "ohos")]
        {
            if let Some(ref client) = self.client {
                let client = client.clone();
                let sid = shortcut.id();
                std::thread::spawn(move || {
                    if let Err(e) = futures_executor::block_on(client.unregister(sid)) {
                        log::error!("[global-shortcut] Failed to unregister shortcut {}: {:?}", sid, e);
                    }
                });
            }
        }

        self.shortcuts.lock().unwrap_or_else(|e| e.into_inner()).remove(&shortcut.id());
        Ok(())
    }

    /// Unregister multiple shortcuts.
    pub fn unregister_multiple<T: TryInto<ShortcutWrapper>, S: IntoIterator<Item = T>>(
        &self,
        shortcuts: S,
    ) -> Result<()>
    where
        T::Error: std::error::Error,
    {
        let mut mapped_shortcuts = Vec::new();
        for shortcut in shortcuts {
            mapped_shortcuts.push(try_into_shortcut(shortcut)?);
        }

        #[cfg(not(target_env = "ohos"))]
        {
            let mapped_clone = mapped_shortcuts.clone();
            #[rustfmt::skip]
            run_main_thread!(self.app, self.manager, |m| m.0.unregister_all(&mapped_clone))?;
        }

        #[cfg(target_env = "ohos")]
        {
            if let Some(ref client) = self.client {
                for s in &mapped_shortcuts {
                    let client = client.clone();
                    let sid = s.id();
                    std::thread::spawn(move || {
                        if let Err(e) = futures_executor::block_on(client.unregister(sid)) {
                            log::error!("[global-shortcut] Failed to unregister shortcut {}: {:?}", sid, e);
                        }
                    });
                }
            }
        }

        let mut shortcuts = self.shortcuts.lock().unwrap_or_else(|e| e.into_inner());
        for s in mapped_shortcuts {
            shortcuts.remove(&s.id());
        }

        Ok(())
    }

    /// Unregister all registered shortcuts.
    pub fn unregister_all(&self) -> Result<()> {
        let mut shortcuts = self.shortcuts.lock().unwrap_or_else(|e| e.into_inner());
        let hotkeys = std::mem::take(&mut *shortcuts);

        #[cfg(not(target_env = "ohos"))]
        {
            let hotkey_vec = hotkeys.values().map(|s| s.shortcut.clone()).collect::<Vec<_>>();
            #[rustfmt::skip]
            let res = run_main_thread!(self.app, self.manager, |m| m.0.unregister_all(hotkey_vec.as_slice()));
            res.map_err(Into::into)
        }

        #[cfg(target_env = "ohos")]
        {
            let _ = &hotkeys; // suppress unused warning on OHOS
            if let Some(ref client) = self.client {
                let client = client.clone();
                std::thread::spawn(move || {
                    if let Err(e) = futures_executor::block_on(client.unregister_all()) {
                        log::error!("[global-shortcut] Failed to unregister all shortcuts: {:?}", e);
                    }
                });
            }
            Ok(())
        }
    }

    /// Determines whether the given shortcut is registered by this application or not.
    ///
    /// If the shortcut is registered by another application, it will still return `false`.
    ///
    /// # OHOS note
    /// Registration is fire-and-forget: `register()` returns immediately after
    /// queueing the request, while the actual `inputConsumer.on()` call happens
    /// asynchronously on the ArkTS main thread. This function queries the local
    /// `shortcuts` map, so it may return `true` before the ArkTS registration
    /// has completed (or even if ArkTS registration ultimately fails). Blocking
    /// to wait for confirmation would risk a deadlock with the main thread, so
    /// this timing gap is an accepted trade-off.
    pub fn is_registered<S: TryInto<ShortcutWrapper>>(&self, shortcut: S) -> bool
    where
        S::Error: std::error::Error,
    {
        if let Ok(shortcut) = try_into_shortcut(shortcut) {
            self.shortcuts.lock().unwrap_or_else(|e| e.into_inner()).contains_key(&shortcut.id())
        } else {
            false
        }
    }
}

pub trait GlobalShortcutExt<R: Runtime> {
    fn global_shortcut(&self) -> &GlobalShortcut<R>;
}

impl<R: Runtime, T: Manager<R>> GlobalShortcutExt<R> for T {
    fn global_shortcut(&self) -> &GlobalShortcut<R> {
        self.state::<GlobalShortcut<R>>().inner()
    }
}

fn parse_shortcut<S: AsRef<str>>(shortcut: S) -> Result<Shortcut> {
    #[cfg(not(target_env = "ohos"))]
    {
        shortcut.as_ref().parse().map_err(Into::into)
    }
    #[cfg(target_env = "ohos")]
    {
        shortcut
            .as_ref()
            .parse::<Shortcut>()
            .map_err(|e| Error::GlobalHotkey(e))
    }
}

fn try_into_shortcut<S: TryInto<ShortcutWrapper>>(shortcut: S) -> Result<Shortcut>
where
    S::Error: std::error::Error,
{
    shortcut
        .try_into()
        .map(|s| s.0)
        .map_err(|e| Error::GlobalHotkey(e.to_string()))
}

#[derive(Clone, Serialize)]
struct ShortcutJsEvent {
    shortcut: String,
    id: u32,
    state: ShortcutState,
}

#[tauri::command]
fn register<R: Runtime>(
    _app: AppHandle<R>,
    global_shortcut: State<'_, GlobalShortcut<R>>,
    shortcuts: Vec<String>,
    handler: Channel<ShortcutJsEvent>,
) -> Result<()> {
    let mut hotkeys = Vec::new();

    let mut shortcut_map = HashMap::new();
    for shortcut in shortcuts {
        let hotkey = parse_shortcut(&shortcut)?;
        shortcut_map.insert(hotkey.id(), shortcut);
        hotkeys.push(hotkey);
    }

    global_shortcut.register_multiple_internal(
        hotkeys,
        Some(
            move |_app: &AppHandle<R>, shortcut: &Shortcut, e: ShortcutEvent| {
                let js_event = ShortcutJsEvent {
                    id: e.id,
                    state: e.state,
                    shortcut: shortcut.into_string(),
                };
                let _ = handler.send(js_event);
            },
        ),
    )
}

#[tauri::command]
fn unregister<R: Runtime>(
    _app: AppHandle<R>,
    global_shortcut: State<'_, GlobalShortcut<R>>,
    shortcuts: Vec<String>,
) -> Result<()> {
    let mut hotkeys = Vec::new();
    for shortcut in shortcuts {
        hotkeys.push(parse_shortcut(&shortcut)?);
    }
    global_shortcut.unregister_multiple(hotkeys)
}

#[tauri::command]
fn unregister_all<R: Runtime>(
    _app: AppHandle<R>,
    global_shortcut: State<'_, GlobalShortcut<R>>,
) -> Result<()> {
    global_shortcut.unregister_all()
}

#[tauri::command]
fn is_registered<R: Runtime>(
    _app: AppHandle<R>,
    global_shortcut: State<'_, GlobalShortcut<R>>,
    shortcut: String,
) -> Result<bool> {
    Ok(global_shortcut.is_registered(parse_shortcut(shortcut)?))
}

pub struct Builder<R: Runtime> {
    shortcuts: Vec<Shortcut>,
    handler: Option<HandlerFn<R>>,
}

impl<R: Runtime> Default for Builder<R> {
    fn default() -> Self {
        Self {
            shortcuts: Vec::new(),
            handler: Default::default(),
        }
    }
}

impl<R: Runtime> Builder<R> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a shortcut to be registerd.
    pub fn with_shortcut<T>(mut self, shortcut: T) -> Result<Self>
    where
        T: TryInto<ShortcutWrapper>,
        T::Error: std::error::Error,
    {
        self.shortcuts.push(try_into_shortcut(shortcut)?);
        Ok(self)
    }

    /// Add multiple shortcuts to be registerd.
    pub fn with_shortcuts<S, T>(mut self, shortcuts: S) -> Result<Self>
    where
        S: IntoIterator<Item = T>,
        T: TryInto<ShortcutWrapper>,
        T::Error: std::error::Error,
    {
        for shortcut in shortcuts {
            self.shortcuts.push(try_into_shortcut(shortcut)?);
        }

        Ok(self)
    }

    /// Specify a global shortcut handler that will be triggered for any and all shortcuts.
    pub fn with_handler<F: Fn(&AppHandle<R>, &Shortcut, ShortcutEvent) + Send + Sync + 'static>(
        mut self,
        handler: F,
    ) -> Self {
        self.handler.replace(Box::new(handler));
        self
    }

    pub fn build(self) -> TauriPlugin<R> {
        let handler = self.handler;
        let shortcuts = self.shortcuts;
        PluginBuilder::new("global-shortcut")
            .invoke_handler(tauri::generate_handler![
                register,
                unregister,
                unregister_all,
                is_registered,
            ])
            .setup(move |app, _api| {
                let mut store = HashMap::<HotKeyId, RegisteredShortcut<R>>::new();

                // ─── Desktop: use global-hotkey ──────────────────────────
                #[cfg(not(target_env = "ohos"))]
                {
                    let manager = global_hotkey::GlobalHotKeyManager::new()?;
                    for shortcut in shortcuts {
                        manager.register(shortcut.clone())?;
                        store.insert(
                            shortcut.id(),
                            RegisteredShortcut {
                                shortcut,
                                handler: None,
                            },
                        );
                    }

                    let shortcuts = Arc::new(Mutex::new(store));
                    let shortcuts_ = shortcuts.clone();

                    let app_handle = app.clone();
                    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
                        if let Some(shortcut) = shortcuts_.lock().unwrap_or_else(|err| err.into_inner()).get(&e.id) {
                            if let Some(handler) = &shortcut.handler {
                                handler(&app_handle, &shortcut.shortcut, e.clone());
                            }
                            if let Some(handler) = &handler {
                                handler(&app_handle, &shortcut.shortcut, e);
                            }
                        }
                    }));

                    app.manage(GlobalShortcut {
                        app: app.clone(),
                        manager: Arc::new(GlobalHotKeyManager(manager)),
                        shortcuts,
                    });
                }

                // ─── OHOS: use openharmony-ability ───────────────────────
                #[cfg(target_env = "ohos")]
                ohos_setup(app.clone(), shortcuts, handler, store);

                Ok(())
            })
            .build()
    }
}
