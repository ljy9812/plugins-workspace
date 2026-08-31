// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{error::Error, Result};

#[tauri::command]
pub async fn is_continuation_restore() -> Result<bool> {
    Err(Error::Unsupported)
}

#[tauri::command]
pub async fn get_continuation_data() -> Result<Option<String>> {
    Err(Error::Unsupported)
}

#[tauri::command]
pub async fn set_continuation_data(data: String) -> Result<()> {
    Err(Error::Unsupported)
}
