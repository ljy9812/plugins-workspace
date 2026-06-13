// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::sync::Mutex;

use crate::SingleInstanceCallback;
use tauri::{
    plugin::{self, TauriPlugin},
    AppHandle, Manager, RunEvent, Runtime,
};

pub fn init<R: Runtime>(cb: Box<SingleInstanceCallback<R>>) -> TauriPlugin<R> {
    let cb = Mutex::new(cb);
    plugin::Builder::new("single-instance")
        .setup(|_app, _api| {
            // OHOS enforces singleton at the OS level via launchType: "singleton".
            // No socket/mutex/D-Bus needed.
            Ok(())
        })
        .on_event(move |app, event| {
            // First launch does NOT trigger this handler:
            // tao emits Event::Opened only on onNewWant (re-launch),
            // not on initial onCreate. This guarantees the callback
            // fires only when a second instance is attempted.
            if let RunEvent::Opened { urls } = event {
                let params_json = openharmony_ability::take_want_parameters();
                let uri = urls
                    .first()
                    .map(|u| u.to_string())
                    .unwrap_or_default();
                // OHOS: args = [uri?, want_parameters_json?]
                // Unlike Linux/Windows (CLI args) or macOS (URLs), OHOS passes
                // the deep-link URI and the Want parameters JSON from onNewWant.
                // cwd is always empty on OHOS (no filesystem cwd concept).
                let args: Vec<String> = [uri, params_json]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect();
                match cb.lock() {
                    Ok(mut cb) => cb(app.app_handle(), args, String::new()),
                    Err(e) => tracing::error!("single-instance callback mutex poisoned: {e}"),
                }
            }

            if let RunEvent::Exit = event {
                destroy(app);
            }
        })
        .build()
}

pub fn destroy<R: Runtime, M: Manager<R>>(_manager: &M) {
    // No-op: OHOS has no socket/mutex/D-Bus to clean up.
}
