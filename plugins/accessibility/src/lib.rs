// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#![cfg(not(any(target_os = "android", target_os = "ios")))]

#[cfg(not(target_env = "ohos"))]
mod commands;
#[cfg(target_env = "ohos")]
mod ohos;

mod error;

pub use error::{Error, Result};

use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    Runtime,
};

#[derive(Default)]
pub struct Builder;

impl Builder {
    pub fn new() -> Self {
        Self
    }

    #[cfg(target_env = "ohos")]
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        PluginBuilder::new("accessibility")
            .invoke_handler(tauri::generate_handler![
                ohos::get_font_scale,
                ohos::is_screen_reader_enabled,
                ohos::is_touch_explore_enabled,
            ])
            .setup(|app, _api| {
                // Event forwarding (bridge subscription + emit) is fire-and-forget:
                // registration failure only disables the state-change event, the query
                // commands keep working.
                setup_event_forwarding(app.clone());
                Ok(())
            })
            .build()
    }

    #[cfg(not(target_env = "ohos"))]
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        PluginBuilder::new("accessibility")
            .invoke_handler(tauri::generate_handler![
                commands::get_font_scale,
                commands::is_screen_reader_enabled,
                commands::is_touch_explore_enabled,
            ])
            .build()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new().build()
}

/// Name of the Tauri event carrying the screen-reader state (a boolean payload).
pub const STATE_CHANGE_EVENT: &str = "accessibility-state-changed";

#[cfg(target_env = "ohos")]
fn setup_event_forwarding<R: Runtime>(app_handle: tauri::AppHandle<R>) {
    use openharmony_ability_plugin_accessibility::AccessibilityBridgePlugin;

    // Register the Rust-side accessibility bridge plugin so ArkTS configurePlugins can
    // match it. Without this, bridge calls fail with "not installed for '<module>'".
    if let Ok(guard) = tauri::ohos::APP.lock() {
        if let Some(ohos_app) = guard.as_ref() {
            if let Err(e) = ohos_app.register_plugin(AccessibilityBridgePlugin) {
                log::error!(
                    "[accessibility] failed to register AccessibilityBridgePlugin: {}",
                    e
                );
            }
        }
    }

    // Plugin setup runs on the main thread — subscribing is a bridge call, so it MUST
    // NOT be awaited here. Do it on a worker thread; the handler itself runs on the
    // NAPI main thread and only emits (non-blocking).
    std::thread::spawn(move || {
        use openharmony_ability_plugin_accessibility::AccessibilityExt;

        let client = tauri::ohos::APP
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|app| app.clone()))
            .and_then(|app| app.accessibility().ok());
        let Some(client) = client else {
            log::error!("[accessibility] OHOS APP not initialized; state-change event disabled");
            return;
        };

        let result = futures_executor::block_on(client.subscribe_state_change(move |enabled| {
            use tauri::Emitter;
            let _ = app_handle.emit(STATE_CHANGE_EVENT, enabled);
        }));
        if let Err(e) = result {
            log::error!("[accessibility] state-change subscription failed: {}", e);
        }
    });
}
