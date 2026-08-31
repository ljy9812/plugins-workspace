// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{error::Error, Result};

/// Builds an `AccessibilityClient` bound to the process-wide OHOS app.
///
/// The `APP` mutex guard is fully dropped before this returns (the client holds its
/// own `BridgeRuntime` clone), so callers may `.await` freely — the `!Send` guard
/// never crosses an await point.
fn client() -> Result<openharmony_ability_plugin_accessibility::AccessibilityClient> {
    use openharmony_ability_plugin_accessibility::AccessibilityExt;

    let guard = tauri::ohos::APP
        .lock()
        .map_err(|_| Error::Accessibility("OHOS APP mutex poisoned".to_string()))?;
    let app = guard
        .as_ref()
        .ok_or_else(|| Error::Accessibility("OHOS APP not initialized".to_string()))?;
    app.accessibility()
        .map_err(|e| Error::from_napi_reason(&e.reason))
}

/// Returns the system font scale from the ability `Configuration` (default 1.0).
/// Requires no permission.
#[tauri::command]
pub async fn get_font_scale() -> Result<f64> {
    let client = client()?;
    client
        .get_font_scale()
        .await
        .map_err(|e| Error::from_napi_reason(&e.reason))
}

/// Returns whether a screen reader is currently open.
///
/// OHOS documents `ohos.permission.ACCESSIBILITY` (system_core) for this query; a
/// third-party denial rejects with a structured error rather than a silent `false`.
#[tauri::command]
pub async fn is_screen_reader_enabled() -> Result<bool> {
    let client = client()?;
    client
        .is_open_accessibility()
        .await
        .map_err(|e| Error::from_napi_reason(&e.reason))
}

/// Returns whether touch exploration (touch guide) is enabled.
#[tauri::command]
pub async fn is_touch_explore_enabled() -> Result<bool> {
    let client = client()?;
    client
        .is_touch_explore_enabled()
        .await
        .map_err(|e| Error::from_napi_reason(&e.reason))
}
