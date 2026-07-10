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
                Modifiers::CONTROL => "Ctrl",
                Modifiers::ALT => "Alt",
                Modifiers::SHIFT => "Shift",
                Modifiers::SUPER => "Meta",
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

#[cfg(not(target_env = "ohos"))]
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

#[cfg(target_env = "ohos")]
impl From<Shortcut> for ShortcutWrapper {
    fn from(value: Shortcut) -> Self {
        Self(value)
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

/// Convert OHOS stub modifiers to openharmony_ability ShortcutModifier.
#[cfg(target_env = "ohos")]
fn to_ohos_modifiers(modifiers: &[Modifiers]) -> Vec<openharmony_ability::ShortcutModifier> {
    modifiers
        .iter()
        .map(|m| match m {
            Modifiers::CONTROL => openharmony_ability::ShortcutModifier::Control,
            Modifiers::ALT => openharmony_ability::ShortcutModifier::Alt,
            Modifiers::SHIFT => openharmony_ability::ShortcutModifier::Shift,
            Modifiers::SUPER => openharmony_ability::ShortcutModifier::Super,
        })
        .collect()
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
            let ohos_modifiers = to_ohos_modifiers(shortcut.modifiers());

            let ohos_key = openharmony_ability::ShortcutKey::from_name(shortcut.code().to_ohos_name())
                .ok_or_else(|| Error::GlobalHotkey(format!("Unknown key: {}", shortcut.code().to_ohos_name())))?;

            openharmony_ability::register_shortcut(&ohos_modifiers, ohos_key, id)
                .map_err(|e| Error::GlobalHotkey(e))?;
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
                let ohos_modifiers = to_ohos_modifiers(shortcut.modifiers());

                let ohos_key = openharmony_ability::ShortcutKey::from_name(shortcut.code().to_ohos_name())
                    .ok_or_else(|| Error::GlobalHotkey(format!("Unknown key: {}", shortcut.code().to_ohos_name())))?;
                openharmony_ability::register_shortcut(&ohos_modifiers, ohos_key, shortcut.id())
                    .map_err(|e| Error::GlobalHotkey(e))?;
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
            if let Err(e) = openharmony_ability::unregister_shortcut(shortcut.id()) {
                log::warn!("Failed to unregister shortcut {}: {}", shortcut.id(), e);
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
            for s in &mapped_shortcuts {
                if let Err(e) = openharmony_ability::unregister_shortcut(s.id()) {
                    log::warn!("Failed to unregister shortcut {}: {}", s.id(), e);
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
            openharmony_ability::unregister_all_shortcuts()
                .map_err(|e| Error::GlobalHotkey(e.to_string()))
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
                {
                    // Initialize the forwarder (must be called before register_shortcut)
                    let app_clone = app.clone();
                    openharmony_ability::init_forwarder(move |task| {
                        let _ = app_clone.run_on_main_thread(move || task());
                    });

                    for shortcut in shortcuts {
                        let ohos_modifiers = to_ohos_modifiers(shortcut.modifiers());

                        if let Some(ohos_key) =
                            openharmony_ability::ShortcutKey::from_name(shortcut.code().to_ohos_name())
                        {
                            if openharmony_ability::register_shortcut(
                                &ohos_modifiers,
                                ohos_key,
                                shortcut.id(),
                            ).is_ok() {
                                store.insert(
                                    shortcut.id(),
                                    RegisteredShortcut {
                                        shortcut,
                                        handler: None,
                                    },
                                );
                            } else {
                                log::warn!("Failed to register shortcut: {}", shortcut.id());
                            }
                        }
                    }

                    let shortcuts = Arc::new(Mutex::new(store));
                    let shortcuts_ = shortcuts.clone();
                    let app_handle = app.clone();

                    // Spawn thread to receive shortcut events from openharmony-ability.
                    // This thread runs for the entire app lifetime with no shutdown signal.
                    // It is cleaned up by the OS when the process exits. The blocking recv()
                    // loop terminates only when the sender side is dropped (i.e., on process
                    // teardown). For a graceful shutdown, call unregister_all() before exit.
                    std::thread::spawn(move || {
                        let receiver = openharmony_ability::shortcut_event_receiver();
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
                                    state: match event.state {
                                        openharmony_ability::ShortcutState::Pressed => {
                                            ShortcutState::Pressed
                                        }
                                        openharmony_ability::ShortcutState::Released => {
                                            ShortcutState::Released
                                        }
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

                    app.manage(GlobalShortcut {
                        shortcuts,
                    });
                }

                Ok(())
            })
            .build()
    }
}
