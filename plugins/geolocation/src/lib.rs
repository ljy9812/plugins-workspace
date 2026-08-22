// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(all(desktop, not(target_env = "ohos")))]
mod desktop;
#[cfg(any(mobile, target_env = "ohos"))]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(all(desktop, not(target_env = "ohos")))]
pub use desktop::Geolocation;
#[cfg(any(mobile, target_env = "ohos"))]
pub use mobile::Geolocation;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`], [`tauri::WebviewWindow`], [`tauri::Webview`] and [`tauri::Window`] to access the geolocation APIs.
pub trait GeolocationExt<R: Runtime> {
    fn geolocation(&self) -> &Geolocation<R>;
}

impl<R: Runtime, T: Manager<R>> crate::GeolocationExt<R> for T {
    fn geolocation(&self) -> &Geolocation<R> {
        self.state::<Geolocation<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("geolocation")
        .invoke_handler(tauri::generate_handler![
            commands::get_current_position,
            commands::watch_position,
            commands::clear_watch,
            commands::check_permissions,
            commands::request_permissions,
            #[cfg(target_env = "ohos")]
            commands::open_location_settings
        ])
        .setup(|app, api| {
            #[cfg(any(mobile, target_env = "ohos"))]
            let geolocation = mobile::init(app, api)?;
            #[cfg(all(desktop, not(target_env = "ohos")))]
            let geolocation = desktop::init(app, api)?;
            app.manage(geolocation);
            Ok(())
        })
        .build()
}
