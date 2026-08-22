// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Save window positions and sizes and restore them when the app is reopened.

#![doc(
    html_logo_url = "https://github.com/tauri-apps/tauri/raw/dev/app-icon.png",
    html_favicon_url = "https://github.com/tauri-apps/tauri/raw/dev/app-icon.png"
)]
#![cfg(not(any(target_os = "android", target_os = "ios")))]

use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    AppHandle, Manager, Monitor, PhysicalPosition, PhysicalSize, RunEvent, Runtime, WebviewWindow,
    Window, WindowEvent,
};

use std::{
    collections::{HashMap, HashSet},
    fs::create_dir_all,
    io::BufReader,
    sync::{Arc, Mutex},
};

mod cmd;

type LabelMapperFn = dyn Fn(&str) -> &str + Send + Sync;
type FilterCallbackFn = dyn Fn(&str) -> bool + Send + Sync;

/// Default filename used to store window state.
///
/// If using a custom filename, you should probably use [`AppHandleExt::filename`] instead.
pub const DEFAULT_FILENAME: &str = ".window-state.json";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct StateFlags: u32 {
        const SIZE        = 1 << 0;
        const POSITION    = 1 << 1;
        const MAXIMIZED   = 1 << 2;
        const VISIBLE     = 1 << 3;
        const DECORATIONS = 1 << 4;
        const FULLSCREEN  = 1 << 5;
    }
}

impl Default for StateFlags {
    /// Default to [`all`](Self::all)
    fn default() -> Self {
        Self::all()
    }
}

struct PluginState {
    pub(crate) state_flags: StateFlags,
    filename: String,
    map_label: Option<Box<LabelMapperFn>>,
    denylist: HashSet<String>,
    filter_callback: Option<Box<FilterCallbackFn>>,
    skip_initial_state: HashSet<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
struct WindowState {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    // prev_x and prev_y are used to store position
    // before maximization happened, because maximization
    // will set x and y to the top-left corner of the monitor
    prev_x: i32,
    prev_y: i32,
    maximized: bool,
    visible: bool,
    decorated: bool,
    fullscreen: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: Default::default(),
            height: Default::default(),
            x: Default::default(),
            y: Default::default(),
            prev_x: Default::default(),
            prev_y: Default::default(),
            maximized: Default::default(),
            visible: true,
            decorated: true,
            fullscreen: Default::default(),
        }
    }
}

struct WindowStateCache(Arc<Mutex<HashMap<String, WindowState>>>);
/// Used to prevent deadlocks from resize and position event listeners setting the cached state on restoring states
struct RestoringWindowState(Mutex<()>);

pub trait AppHandleExt {
    /// Saves all open windows state to disk
    fn save_window_state(&self, flags: StateFlags) -> Result<()>;
    /// Get the name of the file used to store window state.
    fn filename(&self) -> String;
}

impl<R: Runtime> AppHandleExt for tauri::AppHandle<R> {
    fn save_window_state(&self, flags: StateFlags) -> Result<()> {
        let app_dir = self.path().app_config_dir()?;
        let plugin_state = self.state::<PluginState>();
        let state_path = app_dir.join(&plugin_state.filename);
        let windows = self.webview_windows();
        let cache = self.state::<WindowStateCache>();
        let mut state = cache.0.lock().unwrap();

        // OHOS: skip update_state() — it calls window_getter! (inner_size,
        // outer_position, is_fullscreen, etc.) which sends a message to the
        // main thread and blocks on rx.recv(). From a tokio worker thread
        // (save_window_state is async fn), this goes through
        // proxy.send_event + waker TSFN. The waker may not trigger a prompt
        // MainEvent::UserEvent drain, leaving the worker blocked indefinitely
        // → "failed to receive message from webview".
        //
        // The cache is already kept up to date by the Resized/Moved event
        // handlers (which run on the main thread and only update width/height
        // and x/y). No re-query is needed before writing to disk.
        #[cfg(not(target_env = "ohos"))]
        for (label, s) in state.iter_mut() {
            let window = if let Some(map) = &plugin_state.map_label {
                windows
                    .iter()
                    .find_map(|(l, window)| (map(l) == label).then_some(window))
            } else {
                windows.get(label)
            };

            if let Some(window) = window {
                window.update_state(s, flags)?;
            }
        }
        // OHOS: tao emits `windowRectChange` (MOVE/DRAG) as `Event::ContentRectChange`
        // (lifecycle.rs window_rect_change), NOT `WindowEvent::Moved`. So the Moved handler
        // above — the only place `state.x/y` are refreshed post-creation — never fires on
        // OHOS, leaving them at the creation-time default. save would then write that stale
        // position and restore_state yanks the window back to the creation spot
        // ("save resets position"). `outer_position()` reads the `window_rect` cache that
        // ArkTS keeps current via `on("windowRectChange", MOVE)` during drag (non-blocking,
        // no main-thread recv), so refresh position here, mirroring the non-OHOS label
        // mapping. Other fields (size via the Resized handler, maximized on close) have
        // their own OHOS-safe paths; only POSITION is confirmed broken.
        #[cfg(target_env = "ohos")]
        for (label, s) in state.iter_mut() {
            if !flags.contains(StateFlags::POSITION) {
                continue;
            }
            let window = if let Some(map) = &plugin_state.map_label {
                windows
                    .iter()
                    .find_map(|(l, window)| (map(l) == label).then_some(window))
            } else {
                windows.get(label)
            };
            if let Some(window) = window {
                if let Ok(pos) = window.outer_position() {
                    s.x = pos.x;
                    s.y = pos.y;
                }
            }
        }

        create_dir_all(app_dir)?;
        std::fs::write(state_path, serde_json::to_vec_pretty(&*state)?)?;

        Ok(())
    }

    fn filename(&self) -> String {
        self.state::<PluginState>().filename.clone()
    }
}

pub trait WindowExt {
    /// Restores this window state from disk
    fn restore_state(&self, flags: StateFlags) -> tauri::Result<()>;
}

impl<R: Runtime> WindowExt for WebviewWindow<R> {
    fn restore_state(&self, flags: StateFlags) -> tauri::Result<()> {
        self.as_ref().window().restore_state(flags)
    }
}

impl<R: Runtime> WindowExt for Window<R> {
    fn restore_state(&self, flags: StateFlags) -> tauri::Result<()> {
        let plugin_state = self.app_handle().state::<PluginState>();
        let label = plugin_state
            .map_label
            .as_ref()
            .map(|map| map(self.label()))
            .unwrap_or_else(|| self.label());

        let restoring_window_state = self.state::<RestoringWindowState>();
        let _restoring_window_lock = restoring_window_state.0.lock().unwrap();
        let cache = self.state::<WindowStateCache>();
        let mut c = cache.0.lock().unwrap();

        // OHOS: the window is created by the OS ability (at a default position)
        // before this plugin's on_window_ready fires. The Moved event from window
        // creation overwrites the cache (populated from file during setup) with
        // the default position. Re-read the saved state from the file to restore
        // the correct position. (Desktop platforms are unaffected: tao controls
        // window creation, so Moved fires after restore_state, where the
        // RestoringWindowState lock prevents cache overwrite.)
        #[cfg(target_env = "ohos")]
        {
            if let Ok(app_dir) = self.app_handle().path().app_config_dir() {
                let state_path = app_dir.join(&plugin_state.filename);
                if state_path.exists() {
                    if let Ok(data) = std::fs::read(&state_path) {
                        if let Ok(file_cache) =
                            serde_json::from_slice::<HashMap<String, WindowState>>(&data)
                        {
                            if let Some(saved) = file_cache.get(label) {
                                c.insert(label.to_string(), saved.clone());
                            }
                        }
                    }
                }
            }
        }

        let mut should_show = true;

        if let Some(state) = c
            .get(label)
            .filter(|state| state != &&WindowState::default())
        {
            if flags.contains(StateFlags::DECORATIONS) {
                #[cfg(desktop)]
                self.set_decorations(state.decorated)?;
            }

            if flags.contains(StateFlags::POSITION) {
                let position = (state.x, state.y).into();
                let size = (state.width, state.height).into();
                for m in self.available_monitors()? {
                    if m.intersects(position, size) {
                        self.set_position(PhysicalPosition {
                            x: if state.maximized { state.prev_x } else { state.x },
                            y: if state.maximized { state.prev_y } else { state.y },
                        })?;
                    }
                }
            }

            if flags.contains(StateFlags::SIZE) {
                self.set_size(PhysicalSize {
                    width: state.width,
                    height: state.height,
                })?;
            }

            if flags.contains(StateFlags::MAXIMIZED) && state.maximized {
                #[cfg(desktop)]
                self.maximize()?;
            }

            if flags.contains(StateFlags::FULLSCREEN) {
                #[cfg(desktop)]
                self.set_fullscreen(state.fullscreen)?;
            }

            should_show = state.visible;
        } else {
            let mut metadata = WindowState::default();

            // OHOS: skip window_getter! calls (inner_size, outer_position,
            // is_maximized, etc.) — they block on rx.recv() from a tokio
            // worker thread. Default values (0×0, not maximized, etc.) are
            // acceptable; the Resized/Moved event handlers will populate
            // the cache with real values on subsequent events.
            #[cfg(not(target_env = "ohos"))]
            {
            if flags.contains(StateFlags::SIZE) {
                let size = self.inner_size()?;
                metadata.width = size.width;
                metadata.height = size.height;
            }

            if flags.contains(StateFlags::POSITION) {
                let pos = self.outer_position()?;
                metadata.x = pos.x;
                metadata.y = pos.y;
            }

            if flags.contains(StateFlags::MAXIMIZED) {
                metadata.maximized = self.is_maximized()?;
            }

            if flags.contains(StateFlags::VISIBLE) {
                metadata.visible = self.is_visible()?;
            }

            if flags.contains(StateFlags::DECORATIONS) {
                metadata.decorated = self.is_decorated()?;
            }

            if flags.contains(StateFlags::FULLSCREEN) {
                metadata.fullscreen = self.is_fullscreen()?;
            }
            } // end #[cfg(not(target_env = "ohos"))]

            c.insert(label.into(), metadata);
        }

        if flags.contains(StateFlags::VISIBLE) && should_show {
            self.show()?;
            self.set_focus()?;
        }

        Ok(())
    }
}

trait WindowExtInternal {
    fn update_state(&self, state: &mut WindowState, flags: StateFlags) -> tauri::Result<()>;
}

impl<R: Runtime> WindowExtInternal for WebviewWindow<R> {
    fn update_state(&self, state: &mut WindowState, flags: StateFlags) -> tauri::Result<()> {
        self.as_ref().window().update_state(state, flags)
    }
}

impl<R: Runtime> WindowExtInternal for Window<R> {
    fn update_state(&self, state: &mut WindowState, flags: StateFlags) -> tauri::Result<()> {
        // OHOS: is_maximized()/is_minimized() use synchronous NAPI calls
        // (is_window_maximized/is_window_minimized) that block the main thread
        // during window transitions (CloseRequested etc.), causing appfreeze /
        // test timeouts (see on_window_event Resized/Moved fix). Skip on OHOS
        // (set false); other queries (is_fullscreen/is_decorated/is_visible /
        // inner_size/outer_position) are default values or cached, non-blocking.
        #[cfg(target_env = "ohos")]
        let is_maximized = false;
        #[cfg(not(target_env = "ohos"))]
        let is_maximized = flags
            .intersects(StateFlags::MAXIMIZED | StateFlags::POSITION | StateFlags::SIZE)
            && self.is_maximized()?;
        #[cfg(target_env = "ohos")]
        let is_minimized = false;
        #[cfg(not(target_env = "ohos"))]
        let is_minimized =
            flags.intersects(StateFlags::POSITION | StateFlags::SIZE) && self.is_minimized()?;

        if flags.contains(StateFlags::MAXIMIZED) {
            state.maximized = is_maximized;
        }

        if flags.contains(StateFlags::FULLSCREEN) {
            state.fullscreen = self.is_fullscreen()?;
        }

        if flags.contains(StateFlags::DECORATIONS) {
            state.decorated = self.is_decorated()?;
        }

        if flags.contains(StateFlags::VISIBLE) {
            state.visible = self.is_visible()?;
        }

        if flags.contains(StateFlags::SIZE) && !is_maximized && !is_minimized {
            let size = self.inner_size()?;
            // It doesn't make sense to save a window with 0 height or width
            if size.width > 0 && size.height > 0 {
                state.width = size.width;
                state.height = size.height;
            }
        }

        if flags.contains(StateFlags::POSITION) && !is_maximized && !is_minimized {
            let position = self.outer_position()?;
            state.x = position.x;
            state.y = position.y;
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct Builder {
    denylist: HashSet<String>,
    filter_callback: Option<Box<FilterCallbackFn>>,
    skip_initial_state: HashSet<String>,
    state_flags: StateFlags,
    map_label: Option<Box<LabelMapperFn>>,
    filename: Option<String>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the state flags to control what state gets restored and saved.
    pub fn with_state_flags(mut self, flags: StateFlags) -> Self {
        self.state_flags = flags;
        self
    }

    /// Sets a custom filename to use when saving and restoring window states from disk.
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename.replace(filename.into());
        self
    }

    /// Sets a list of windows that shouldn't be tracked and managed by this plugin
    /// For example, splash screen windows.
    pub fn with_denylist(mut self, denylist: &[&str]) -> Self {
        self.denylist = denylist.iter().map(|l| l.to_string()).collect();
        self
    }

    /// Sets a filter callback to exclude specific windows from being tracked.
    /// Return `true` to save the state, or `false` to skip and not save it.
    pub fn with_filter<F>(mut self, filter_callback: F) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        self.filter_callback = Some(Box::new(filter_callback));
        self
    }

    /// Adds the given window label to a list of windows to skip initial state restore.
    pub fn skip_initial_state(mut self, label: &str) -> Self {
        self.skip_initial_state.insert(label.into());
        self
    }

    /// Transforms the window label when saving the window state.
    ///
    /// This can be used to group different windows to use the same state.
    pub fn map_label<F>(mut self, map_fn: F) -> Self
    where
        F: Fn(&str) -> &str + Sync + Send + 'static,
    {
        self.map_label = Some(Box::new(map_fn));
        self
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        let state_flags = self.state_flags;
        let filename = self.filename.unwrap_or_else(|| DEFAULT_FILENAME.into());
        let map_label = self.map_label;
        let denylist = self.denylist;
        let filter_callback = self.filter_callback;
        let skip_initial_state = self.skip_initial_state;

        PluginBuilder::new("window-state")
            .invoke_handler(tauri::generate_handler![
                cmd::save_window_state,
                cmd::restore_state,
                cmd::filename
            ])
            .setup(move |app, _api| {
                let cache = load_saved_window_states(app, &filename).unwrap_or_default();
                app.manage(WindowStateCache(Arc::new(Mutex::new(cache))));
                app.manage(RestoringWindowState(Mutex::new(())));
                app.manage(PluginState {
                    state_flags,
                    filename,
                    map_label,
                    denylist,
                    filter_callback,
                    skip_initial_state,
                });
                Ok(())
            })
            .on_window_ready(move |window| {
                let plugin_state = window.app_handle().state::<PluginState>();
                let label = plugin_state
                    .map_label
                    .as_ref()
                    .map(|map| map(window.label()))
                    .unwrap_or_else(|| window.label());

                // Check deny list names
                if plugin_state.denylist.contains(label) {
                    return;
                }

                // Check deny list callback
                if let Some(filter_callback) = &plugin_state.filter_callback {
                    // Don't save the state if the callback returns false
                    if !filter_callback(label) {
                        return;
                    }
                }

                if !plugin_state.skip_initial_state.contains(label) {
                    let _ = window.restore_state(state_flags);
                }

                let cache = window.state::<WindowStateCache>();
                let cache = cache.0.clone();
                let label = label.to_string();
                let window_clone = window.clone();

                // insert a default state if this window should be tracked and
                // the disk cache doesn't have a state for it
                {
                    cache
                        .lock()
                        .unwrap()
                        .entry(label.clone())
                        .or_insert_with(WindowState::default);
                }

                window.on_window_event(move |e| match e {
                    WindowEvent::CloseRequested { .. } => {
                        let mut c = cache.lock().unwrap();
                        if let Some(state) = c.get_mut(&label) {
                            let _ = window_clone.update_state(state, state_flags);
                        }
                    }

                    WindowEvent::Moved(position) if state_flags.contains(StateFlags::POSITION) => {
                        if window_clone
                            .state::<RestoringWindowState>()
                            .0
                            .try_lock()
                            .is_ok()
                        {
                            // OHOS: is_minimized() 经 window_getter! → is_window_minimized 的同步
                            // NAPI 调用,在窗口过渡期(Moved/Resized 事件回调中)阻塞主线程致 appfreeze
                            // (run_on_main_thread + recv 重入死锁)。OHOS 上跳过该守卫——最小化窗口不
                            // 触发 Moved;其他平台保留原同步查询(其 send_user_message 主线程短路后
                            // 直接 handle_user_message,非阻塞)。
                            #[cfg(target_env = "ohos")]
                            let minimized = false;
                            #[cfg(not(target_env = "ohos"))]
                            let minimized = window_clone.is_minimized().unwrap_or_default();
                            if !minimized {
                                let mut c = cache.lock().unwrap();
                                if let Some(state) = c.get_mut(&label) {
                                    state.prev_x = state.x;
                                    state.prev_y = state.y;

                                    state.x = position.x;
                                    state.y = position.y;
                                }
                            }
                        }
                    }
                    WindowEvent::Resized(size) if state_flags.contains(StateFlags::SIZE) => {
                        if window_clone
                            .state::<RestoringWindowState>()
                            .0
                            .try_lock()
                            .is_ok()
                        {
                            // OHOS: is_minimized()/is_maximized() 同步查询在 resize 过渡期阻塞主线程
                            // 致 appfreeze(见 Moved 注释)。OHOS 上跳过守卫无条件保存——最小化不触发
                            // Resized;最大化由 close 时 update_state 捕获 state.maximized 驱动恢复,
                            // 保存的尺寸不影响恢复。其他平台保留原同步查询。
                            #[cfg(target_env = "ohos")]
                            let save = true;
                            #[cfg(not(target_env = "ohos"))]
                            let save = {
                                // TODO: Remove once https://github.com/tauri-apps/tauri/issues/5812 is resolved.
                                let is_maximized = if cfg!(target_os = "macos")
                                    && (!window_clone.is_decorated().unwrap_or_default()
                                        || !window_clone.is_resizable().unwrap_or_default())
                                {
                                    false
                                } else {
                                    window_clone.is_maximized().unwrap_or_default()
                                };
                                !window_clone.is_minimized().unwrap_or_default() && !is_maximized
                            };
                            if save {
                                let mut c = cache.lock().unwrap();
                                if let Some(state) = c.get_mut(&label) {
                                    state.width = size.width;
                                    state.height = size.height;
                                }
                            }
                        }
                    }
                    _ => {}
                });
            })
            .on_event(move |app, event| {
                #[cfg(target_env = "ohos")]
                {
                    if let RunEvent::Ready = &event {
                        // Restore on Ready because on_window_ready does not fire for the
                        // main window on OHOS (created before the plugin registers).
                        // Apply the same denylist / filter / skip_initial_state gating as
                        // on_window_ready so excluded windows (e.g. splash) are not restored.
                        let windows_to_restore: Vec<_> = {
                            let plugin_state = app.state::<PluginState>();
                            app.webview_windows()
                                .into_iter()
                                .filter(|(_, window)| {
                                    let label = plugin_state
                                        .map_label
                                        .as_ref()
                                        .map(|map| map(window.label()))
                                        .unwrap_or_else(|| window.label());
                                    if plugin_state.denylist.contains(label) {
                                        return false;
                                    }
                                    if let Some(filter_callback) = &plugin_state.filter_callback {
                                        if !filter_callback(label) {
                                            return false;
                                        }
                                    }
                                    !plugin_state.skip_initial_state.contains(label)
                                })
                                .map(|(_, w)| w)
                                .collect()
                        };
                        for window in windows_to_restore {
                            let _ = window.restore_state(state_flags);
                        }
                    }
                    // OHOS: skip auto-save on Exit. The user controls persistence
                    // via the explicit save_window_state command (Save/Restore button).
                    // Auto-save on Exit would overwrite the user's explicit save with
                    // the current (possibly moved) position.
                }
                #[cfg(not(target_env = "ohos"))]
                if let RunEvent::Exit = event {
                    let _ = app.save_window_state(state_flags);
                }
            })
            .build()
    }
}

fn load_saved_window_states<R: Runtime>(
    app: &AppHandle<R>,
    filename: &String,
) -> Result<HashMap<String, WindowState>> {
    let app_dir = app.path().app_config_dir()?;
    let state_path = app_dir.join(filename);
    let file = std::fs::File::open(state_path)?;
    let reader = BufReader::new(file);
    let states = serde_json::from_reader(reader)?;
    Ok(states)
}

trait MonitorExt {
    fn intersects(&self, position: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> bool;
}

impl MonitorExt for Monitor {
    fn intersects(&self, position: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> bool {
        let PhysicalPosition { x, y } = *self.position();
        let PhysicalSize { width, height } = *self.size();

        let left = x;
        let right = x + width as i32;
        let top = y;
        let bottom = y + height as i32;

        [
            (position.x, position.y),
            (position.x + size.width as i32, position.y),
            (position.x, position.y + size.height as i32),
            (
                position.x + size.width as i32,
                position.y + size.height as i32,
            ),
        ]
        .into_iter()
        .any(|(x, y)| x >= left && x < right && y >= top && y < bottom)
    }
}
