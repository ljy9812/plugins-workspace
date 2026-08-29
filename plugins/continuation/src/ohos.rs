// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{error::Error, Result};

use openharmony_ability_plugin_continuation::ContinuationClient;

/// wantParam size budget for the continuation payload: ~100 KiB platform limit
/// minus headroom for the surrounding Want fields.
const CONTINUATION_DATA_MAX_BYTES: usize = 96 * 1024;

/// Returns whether the current launch is an app-continuation restore.
///
/// Peek semantics: idempotent, does not consume `get_continuation_data`.
#[tauri::command]
pub async fn is_continuation_restore() -> Result<bool> {
    // The client is zero-sized and stateless — no APP handle, no bridge, no locks
    // held across an await (there are no awaits).
    Ok(ContinuationClient::default().is_continuation_restore())
}

/// Returns the continuation payload JSON from the source device, consuming it.
///
/// Draining take: the first call returns `Some(json)` on a continuation restore,
/// subsequent calls return `None`. `None` also means the launch was not a
/// continuation restore. The payload is passed through verbatim.
#[tauri::command]
pub async fn get_continuation_data() -> Result<Option<String>> {
    let data = ContinuationClient::default().take_continuation_data();
    // Empty string means "no continuation data" — normalize to null for JS.
    Ok((!data.is_empty()).then_some(data))
}

/// Pre-registers the source-side continuation snapshot (overwrite semantics).
///
/// The ArkTS `onContinue` callback reads the snapshot synchronously when the
/// system initiates a migration and forwards it as `wantParam.continuationData`;
/// an empty string clears the snapshot (`onContinue` then refuses with MISMATCH).
/// Reading is peek-only — a cancelled migration leaves the snapshot for a retry.
#[tauri::command]
pub async fn set_continuation_data(data: String) -> Result<()> {
    if data.len() > CONTINUATION_DATA_MAX_BYTES {
        return Err(Error::PayloadTooLarge);
    }
    ContinuationClient::default().set_continuation_data(data);
    Ok(())
}
