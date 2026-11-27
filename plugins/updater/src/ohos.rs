// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! OpenHarmony-specific implementation for the updater plugin.
//!
//! On OHOS, updates are hosted by AppGallery — the desktop updater API
//! (endpoints/pubkey from `tauri.conf.json`) does not apply. Instead of
//! faking the desktop commands, this module exposes:
//! - the cross-platform `check` command, backed by the AppGallery query —
//!   it honors the desktop contract's query half (`null` when no update is
//!   available), while the download half stays AppGallery-owned
//! - `check_app_gallery_update` queries `updateManager.checkAppUpdate()`
//!   (pure metadata, no dialog)
//! - `show_app_gallery_update_dialog` triggers `updateManager.showUpdateDialog()`
//!   (system dialog driving the full download + install flow)
//! - the desktop `download` / `install` commands as explicit
//!   [`Error::UnsupportedPlatform`] stubs, and `download_and_install`
//!   routed to the same AppGallery dialog. Registering them matters: the
//!   cross-platform JS methods invoke `plugin:updater|download` etc.;
//!   without a Rust handler those invokes fall through to the ArkTS
//!   plugin layer — which has no `updater` module — and die as a
//!   confusing "Plugin not found: updater".

use crate::{Error, Result};
use openharmony_ability::Updater;
use serde::Serialize;
use tauri::{Manager, Runtime, Webview};

// ── Types ────────────────────────────────────────────────────────────

/// Result of an AppGallery update check, serialized as
/// `{ available, currentVersion, version }`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppGalleryUpdateInfo {
    available: bool,
    current_version: String,
    version: Option<String>,
}

/// Wire shape of the desktop `UpdateMetadata` (see guest-js) returned by
/// `check` when an update is available. `rid` is the placeholder `0` on
/// OHOS — the desktop download flow does not apply. It is NOT a live
/// resource-table id: the guest's `Update.close()` dispatches
/// `plugin:resources|close` with rid 0, which the resources table rejects
/// as `BadResourceId` (real resource ids are randomly assigned, so a real
/// rid-0 resource is a ~2^-32 coincidence). The update flow is driven by
/// the system dialog (`show_app_gallery_update_dialog`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateMetadata {
    rid: u32,
    current_version: String,
    version: String,
    date: Option<String>,
    body: Option<String>,
    raw_json: serde_json::Value,
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Get the AppGallery updater bridge handle.
///
/// The `tauri::ohos::APP` guard is acquired and dropped inside this helper,
/// so the lock is never held across an await.
fn updater_bridge() -> Result<Updater> {
    let guard = tauri::ohos::APP
        .lock()
        .map_err(|_| Error::Network("OHOS APP mutex poisoned".into()))?;
    let app = guard
        .as_ref()
        .ok_or_else(|| Error::Network("OHOS APP not initialized".into()))?;
    app.updater()
        .map_err(|e| Error::Network(e.reason.to_string()))
}

// ── Commands ─────────────────────────────────────────────────────────

/// Cross-platform `check` command, AppGallery-backed on OHOS.
///
/// Honors the query half of the desktop contract: resolves to `null` when
/// no update is available, or to the update's metadata. The desktop
/// `CheckOptions` (headers/timeout/proxy/target/allowDowngrades) do not
/// apply — AppGallery is the update source — and are ignored, like the
/// options of the two OHOS-only commands below. The download half of the
/// desktop contract is NOT honored: the returned `rid` is the placeholder
/// `0` (see [`UpdateMetadata`]), and the update flow is user-driven via
/// `show_app_gallery_update_dialog`.
///
/// Without this registration the JS `check()` call has no Rust handler, so
/// it falls through to the ArkTS plugin layer — which has no `updater`
/// module — and dies as "Plugin not found: updater".
#[tauri::command]
pub(crate) async fn check<R: Runtime>(_webview: Webview<R>) -> Result<Option<UpdateMetadata>> {
    let updater = updater_bridge()?;

    match updater.check().await {
        // No update available: AppGallery reports nothing, so `check()`
        // resolves to `null` — the desktop contract's no-update semantics.
        Ok(None) => Ok(None),
        Ok(Some(r)) => Ok(Some(UpdateMetadata {
            rid: 0,
            version: r
                .version
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "unknown".into()),
            raw_json: serde_json::json!({
                "source": "appGallery",
                "currentVersion": r.current_version,
            }),
            current_version: r.current_version,
            date: None,
            body: None,
        })),
        Err(e) => Err(Error::Network(e.reason.to_string())),
    }
}

/// Check for updates via AppGallery (OHOS only).
///
/// This is a pure query — no dialog is shown. It takes no options: AppGallery
/// is the update source, so the desktop `check` arguments
/// (headers/timeout/proxy/target/allowDowngrades) do not apply.
#[tauri::command]
pub(crate) async fn check_app_gallery_update<R: Runtime>(
    webview: Webview<R>,
) -> Result<AppGalleryUpdateInfo> {
    let updater = updater_bridge()?;

    match updater.check().await {
        Ok(None) => {
            // No update available: AppGallery reports nothing, so fall back to
            // the bundled version from `Manager::package_info()`.
            Ok(AppGalleryUpdateInfo {
                available: false,
                current_version: webview.package_info().version.to_string(),
                version: None,
            })
        }
        Ok(Some(r)) => Ok(AppGalleryUpdateInfo {
            available: true,
            current_version: r.current_version,
            // The SDK 12 ArkTS fallback is the literal "unknown" (on API < 20
            // the new version number is not available); map that and the empty
            // string to `None` so JS can rely on `version == null`. Since
            // openharmony-ability#51 the bridge itself returns `Option<String>`
            // (null on API < 20), which is passed through here.
            version: r.version.filter(|v| !v.is_empty() && v != "unknown"),
        }),
        Err(e) => Err(Error::Network(e.reason.to_string())),
    }
}

/// Show the system AppGallery update dialog (OHOS only).
///
/// The dialog is user-driven: whether to download and install the update is
/// decided by the user, so resolving `Ok(())` does NOT mean the update was
/// installed.
#[tauri::command]
pub(crate) async fn show_app_gallery_update_dialog<R: Runtime>(_webview: Webview<R>) -> Result<()> {
    updater_bridge()?
        .download_and_install()
        .await
        .map_err(|e| Error::Network(e.reason.to_string()))?;

    Ok(())
}

/// Desktop `download` command, OHOS stub — returns
/// [`Error::UnsupportedPlatform`].
///
/// Updates are AppGallery-owned and the desktop contract (download the
/// bytes into a resources-table entry, emit progress events, resolve with
/// a bytes rid) has no AppGallery equivalent, so the command refuses
/// instead of pretending. The invoke args (`rid` / `onEvent` /
/// `headers` / `timeout`) are accepted-and-ignored: command arguments are
/// extracted per declared parameter, extra payload keys never reach
/// deserialization.
#[tauri::command]
pub(crate) async fn download<R: Runtime>(_webview: Webview<R>) -> Result<tauri::ResourceId> {
    Err(Error::UnsupportedPlatform)
}

/// Desktop `install` command, OHOS stub — returns
/// [`Error::UnsupportedPlatform`].
///
/// Unreachable through the guest bindings in practice (`install()` throws
/// before invoking when `download()` never produced bytes), but registered
/// so a direct invoke gets the explicit refusal instead of the fallback
/// chain's "Plugin not found: updater". Same arg-ignoring note as
/// [`download`].
#[tauri::command]
pub(crate) async fn install<R: Runtime>(_webview: Webview<R>) -> Result<()> {
    Err(Error::UnsupportedPlatform)
}

/// Desktop `download_and_install` command, OHOS mapping — routes to the
/// AppGallery system dialog (same bridge call as
/// [`show_app_gallery_update_dialog`]).
///
/// The dialog is the platform's only sanctioned update flow and drives the
/// whole download + install; whether to proceed is decided by the user, so
/// resolving `Ok(())` does NOT mean the update was installed. The desktop
/// progress events (`onEvent`) have no source in this flow and are never
/// emitted. The invoke args (`rid` / `onEvent` / `headers` / `timeout`)
/// are accepted-and-ignored, see [`download`].
#[tauri::command]
pub(crate) async fn download_and_install<R: Runtime>(_webview: Webview<R>) -> Result<()> {
    updater_bridge()?
        .download_and_install()
        .await
        .map_err(|e| Error::Network(e.reason.to_string()))?;

    Ok(())
}
