// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{error::Error, Result};

#[tauri::command]
pub async fn capture_webview() -> Result<()> {
    Err(Error::Unsupported)
}

#[tauri::command]
pub async fn pick_color(_x: u32, _y: u32) -> Result<()> {
    Err(Error::Unsupported)
}
