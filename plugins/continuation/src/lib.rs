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
        PluginBuilder::new("continuation")
            .invoke_handler(tauri::generate_handler![
                ohos::is_continuation_restore,
                ohos::get_continuation_data,
                ohos::set_continuation_data,
            ])
            .setup(|_app, _api| {
                // No ArkTS plugin and no bridge: the continuation signal was already
                // captured by the NativeAbility lifecycle callbacks into Rust-side
                // storage (Phase 1c). Commands are pure synchronous reads.
                log::info!("[continuation] plugin initialized");
                Ok(())
            })
            .build()
    }

    #[cfg(not(target_env = "ohos"))]
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        PluginBuilder::new("continuation")
            .invoke_handler(tauri::generate_handler![
                commands::is_continuation_restore,
                commands::get_continuation_data,
                commands::set_continuation_data,
            ])
            .build()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new().build()
}
