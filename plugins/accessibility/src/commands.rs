// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{error::Error, Result};

#[tauri::command]
pub async fn get_font_scale() -> Result<f64> {
    Err(Error::Unsupported)
}

#[tauri::command]
pub async fn is_screen_reader_enabled() -> Result<bool> {
    Err(Error::Unsupported)
}

#[tauri::command]
pub async fn is_touch_explore_enabled() -> Result<bool> {
    Err(Error::Unsupported)
}
