// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
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
        PluginBuilder::new("screenshot")
            .invoke_handler(tauri::generate_handler![
                ohos::capture_webview,
                ohos::pick_color,
            ])
            .setup(|_app, _api| {
                // No ArkTS plugin of our own and no state to subscribe: the
                // `capture-webview` / `pick-color` actions ride the globally-registered
                // WebviewBridgePlugin (tauri-runtime-wry registers it), so setup has
                // nothing to do beyond confirming the plugin loaded.
                log::info!("[screenshot] plugin initialized");
                Ok(())
            })
            .build()
    }

    #[cfg(not(target_env = "ohos"))]
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        PluginBuilder::new("screenshot")
            .invoke_handler(tauri::generate_handler![
                commands::capture_webview,
                commands::pick_color,
            ])
            .build()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new().build()
}
