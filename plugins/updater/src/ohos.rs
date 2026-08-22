// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! OpenHarmony-specific implementation for the updater plugin.
//!
//! On OHOS, updates are handled via AppGallery:
//! - `check()` queries `updateManager.checkAppUpdate()` — pure metadata, no dialog
//! - `download_and_install()` triggers `updateManager.showUpdateDialog()` — system dialog
//! - `download()` / `install()` are unsupported (AppGallery handles the full flow)

use crate::{Error, Result};
use serde::Serialize;
use tauri::{ipc::Channel, ResourceId, Runtime, Webview};

// ── Types (mirror commands.rs but self-contained for OHOS) ───────────

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started { content_length: Option<u64> },
    #[serde(rename_all = "camelCase")]
    Progress { chunk_length: usize },
    Finished,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Metadata {
    rid: ResourceId,
    current_version: String,
    version: String,
    date: Option<String>,
    body: Option<String>,
    raw_json: serde_json::Value,
}

// ── Commands ─────────────────────────────────────────────────────────

/// Check for updates via AppGallery (OHOS).
///
/// This is a pure query — no dialog is shown.
/// Returns `None` if no update is available, or `Some(Metadata)` if an update exists.
#[tauri::command]
pub(crate) async fn check<R: Runtime>(
    _webview: Webview<R>,
    _headers: Option<Vec<(String, String)>>,
    _timeout: Option<u64>,
    _proxy: Option<String>,
    _target: Option<String>,
    _allow_downgrades: Option<bool>,
) -> Result<Option<Metadata>> {
    let updater = {
        let guard = tauri::ohos::APP
            .lock()
            .map_err(|_| Error::Network("OHOS APP mutex poisoned".into()))?;
        let app = guard
            .as_ref()
            .ok_or_else(|| Error::Network("OHOS APP not initialized".into()))?;
        app.updater()
            .map_err(|e| Error::Network(e.reason.to_string()))?
    }; // guard dropped before await

    let result = updater
        .check()
        .await
        .map_err(|e| Error::Network(e.reason.to_string()))?;

    Ok(result.map(|r| Metadata {
        rid: ResourceId::from(0u32),
        current_version: r.current_version,
        version: r.version,
        date: r.date,
        body: r.body,
        raw_json: serde_json::Value::Null,
    }))
}

/// Download and install update via AppGallery dialog (OHOS).
///
/// This triggers the system-owned `showUpdateDialog()` which handles
/// the entire download and installation flow.
#[tauri::command]
pub(crate) async fn download_and_install<R: Runtime>(
    _webview: Webview<R>,
    _rid: ResourceId,
    _on_event: Channel<DownloadEvent>,
    _headers: Option<Vec<(String, String)>>,
    _timeout: Option<u64>,
) -> Result<()> {
    let updater = {
        let guard = tauri::ohos::APP
            .lock()
            .map_err(|_| Error::Network("OHOS APP mutex poisoned".into()))?;
        let app = guard
            .as_ref()
            .ok_or_else(|| Error::Network("OHOS APP not initialized".into()))?;
        app.updater()
            .map_err(|e| Error::Network(e.reason.to_string()))?
    }; // guard dropped before await

    updater
        .download_and_install()
        .await
        .map_err(|e| Error::Network(e.reason.to_string()))?;

    Ok(())
}

/// Download update (unsupported on OHOS).
///
/// On OHOS, use `download_and_install()` instead, which triggers the
/// AppGallery system dialog that handles the full update flow.
#[tauri::command]
pub(crate) async fn download<R: Runtime>(
    _webview: Webview<R>,
    _rid: ResourceId,
    _on_event: Channel<DownloadEvent>,
    _headers: Option<Vec<(String, String)>>,
    _timeout: Option<u64>,
) -> Result<ResourceId> {
    Err(Error::UnsupportedPlatform)
}

/// Install downloaded update (unsupported on OHOS).
///
/// On OHOS, use `download_and_install()` instead, which triggers the
/// AppGallery system dialog that handles the full update flow.
#[tauri::command]
pub(crate) async fn install<R: Runtime>(
    _webview: Webview<R>,
    _update_rid: ResourceId,
    _bytes_rid: ResourceId,
) -> Result<()> {
    Err(Error::UnsupportedPlatform)
}
