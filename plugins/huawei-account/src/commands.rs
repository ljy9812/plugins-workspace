// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Desktop (non-OHOS) stubs. Huawei Account Kit is OHOS-only; on other platforms
//! every command returns `Unsupported` without touching any account capability.

use crate::{error::Error, models::AccountInfo};

type Result<T> = std::result::Result<T, Error>;

#[tauri::command]
pub async fn login() -> Result<AccountInfo> {
    Err(Error::Unsupported)
}

#[tauri::command]
pub async fn silent_login() -> Result<AccountInfo> {
    Err(Error::Unsupported)
}

#[tauri::command]
pub async fn logout() -> Result<()> {
    Err(Error::Unsupported)
}
