// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tauri::{AppHandle, Runtime};

#[cfg(not(target_env = "ohos"))]
#[tauri::command]
pub fn restart<R: Runtime>(app: AppHandle<R>) {
    app.request_restart()
}

#[tauri::command]
pub fn exit<R: Runtime>(app: AppHandle<R>, code: i32) {
    app.exit(code)
}
