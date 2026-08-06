// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Huawei account one-tap login for Tauri applications.
//!
//! - Supported platform: OpenHarmony (via `openharmony-ability`'s `account` feature).
//! - Other platforms: commands return `Unsupported`.
//!
//! On OHOS, commands route directly to `openharmony_ability::HuaweiAccount` (p1),
//! bypassing the mobile plugin command pipe. See `design.md` (p2) D1/D2.

#![cfg(not(any(target_os = "android", target_os = "ios")))]

#[cfg(not(target_env = "ohos"))]
mod commands;
mod error;
mod models;
#[cfg(target_env = "ohos")]
mod ohos;

pub use error::{Error, Result};
pub use models::AccountInfo;

use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    Runtime,
};

/// Plugin builder. No configuration is required for huawei-account.
#[derive(Default)]
pub struct Builder;

impl Builder {
    pub fn new() -> Self {
        Self
    }

    #[cfg(target_env = "ohos")]
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        PluginBuilder::new("huawei-account")
            .invoke_handler(tauri::generate_handler![
                ohos::login,
                ohos::silent_login,
                ohos::logout,
            ])
            .build()
    }

    #[cfg(not(target_env = "ohos"))]
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        PluginBuilder::new("huawei-account")
            .invoke_handler(tauri::generate_handler![
                commands::login,
                commands::silent_login,
                commands::logout,
            ])
            .build()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new().build()
}
