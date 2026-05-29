// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! OpenHarmony-specific implementation for the process plugin.

use std::time::Duration;
use tauri::{AppHandle, Runtime};

/// Restart the app on OpenHarmony by calling `appRecovery.restartApp()`
/// through the NAPI bridge (TSFN), bypassing the tao event loop (which does
/// not reliably deliver `RequestExit` on OHOS).
///
/// The process is hard-killed by `restartApp` — `onDestroy` is NOT triggered.
/// After dispatching `restartApp` to the main thread, this thread blocks forever,
/// same pattern as the non-OHOS restart path (let the runtime terminate us).
#[tauri::command]
pub fn restart<R: Runtime>(_app: AppHandle<R>) {
    if let Ok(guard) = tauri::ohos::APP.lock() {
        if let Some(app_ref) = guard.as_ref() {
            match app_ref.restart() {
                Ok(0) => {
                    // restartApp dispatched to main thread; block and let it kill the process
                    loop {
                        std::thread::sleep(Duration::MAX);
                    }
                }
                Ok(code) => log::error!("OHOS restart returned non-zero code: {code}"),
                Err(e) => log::error!("OHOS restart failed: {e}"),
            }
        } else {
            log::error!("OHOS APP not initialized — cannot restart");
        }
    }
    std::process::exit(0);
}
