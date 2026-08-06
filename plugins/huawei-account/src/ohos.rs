// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! OpenHarmony implementation for the huawei-account plugin.
//!
//! Routes directly to `openharmony_ability::HuaweiAccount` (p1 bridge) via plain
//! `#[tauri::command]` + `generate_handler!`, bypassing the mobile plugin command
//! pipe (`run_mobile_plugin` / `dispatch_run_command` / `PENDING_PLUGIN_CALLS`).
//! The TSFNs are initialized during the app's `render()`, so by the time a
//! command is invoked the bridge is ready.

use crate::{error::Error, models::AccountInfo};

type Result<T> = std::result::Result<T, Error>;

/// Interactive login — forces the Huawei account login UI.
#[tauri::command]
pub async fn login() -> Result<AccountInfo> {
    let account = openharmony_ability::HuaweiAccount::new();
    let info = account
        .login()
        .await
        .map_err(|e| Error::from_napi_reason(&e.reason))?;
    Ok(AccountInfo::from(info))
}

/// Silent login — no UI; succeeds only when already logged in & authorized.
#[tauri::command]
pub async fn silent_login() -> Result<AccountInfo> {
    let account = openharmony_ability::HuaweiAccount::new();
    let info = account
        .silent_login()
        .await
        .map_err(|e| Error::from_napi_reason(&e.reason))?;
    Ok(AccountInfo::from(info))
}

/// Logout — cancels the app's Huawei account authorization (p1 design D8).
#[tauri::command]
pub async fn logout() -> Result<()> {
    let account = openharmony_ability::HuaweiAccount::new();
    account
        .logout()
        .await
        .map_err(|e| Error::from_napi_reason(&e.reason))?;
    Ok(())
}
