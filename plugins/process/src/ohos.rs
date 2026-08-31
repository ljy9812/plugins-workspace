// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! OpenHarmony-specific implementation for the process plugin.

/// Restart the app on OpenHarmony.
///
/// The legacy TSFN-based restart helper was removed during decoupling.
/// Process exit triggers the OHOS ability lifecycle restart via the OS.
#[tauri::command]
pub fn restart<R: tauri::Runtime>(_app: tauri::AppHandle<R>) {
    std::process::exit(0);
}
