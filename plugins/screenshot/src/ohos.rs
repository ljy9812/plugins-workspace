// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{error::Error, Result};
use serde::Serialize;

/// A webview screenshot returned to JS (base64 PNG + pixel dimensions).
///
/// The bridge facade's `CapturedImage` is a pure Rust type without serde; this DTO is
/// the wire shape (`camelCase` per plugin convention).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedImageDto {
    pub png_base64: String,
    pub width: u32,
    pub height: u32,
}

impl From<openharmony_ability_plugin_screenshot::CapturedImage> for CapturedImageDto {
    fn from(image: openharmony_ability_plugin_screenshot::CapturedImage) -> Self {
        Self {
            png_base64: image.png_base64,
            width: image.width,
            height: image.height,
        }
    }
}

/// A single pixel's color channels (0-255 each).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RgbaDto {
    pub r: u32,
    pub g: u32,
    pub b: u32,
    pub a: u32,
}

impl From<openharmony_ability_plugin_screenshot::Rgba> for RgbaDto {
    fn from(color: openharmony_ability_plugin_screenshot::Rgba) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

/// Builds a `ScreenshotClient` bound to the process-wide OHOS app.
///
/// The `APP` mutex guard is fully dropped before this returns (the client holds its
/// own `BridgeRuntime` clone), so callers may `.await` freely — the `!Send` guard
/// never crosses an await point.
fn client() -> Result<openharmony_ability_plugin_screenshot::ScreenshotClient> {
    use openharmony_ability_plugin_screenshot::ScreenshotExt;

    let guard = tauri::ohos::APP
        .lock()
        .map_err(|_| Error::Screenshot("OHOS APP mutex poisoned".to_string()))?;
    let app = guard
        .as_ref()
        .ok_or_else(|| Error::Screenshot("OHOS APP not initialized".to_string()))?;
    app.screenshot()
        .map_err(|e| Error::Screenshot(e.reason.clone()))
}

fn map_error(err: openharmony_ability_plugin_screenshot::ScreenshotError) -> Error {
    Error::from_screenshot_message(&err.to_string())
}

/// Captures the calling webview as a base64 PNG with its pixel dimensions.
///
/// The webview is injected by the Tauri command system (the caller); its label is the
/// bridge webview id, so JS never needs to pass an identifier.
#[tauri::command]
pub async fn capture_webview<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
) -> Result<CapturedImageDto> {
    let client = client()?;
    client
        .capture_webview(webview.label())
        .await
        .map(CapturedImageDto::from)
        .map_err(map_error)
}

/// Reads the color of the pixel at snapshot coordinates (`x`, `y`) from the calling
/// webview. Out-of-bounds coordinates reject with a structured error.
#[tauri::command]
pub async fn pick_color<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    x: u32,
    y: u32,
) -> Result<RgbaDto> {
    let client = client()?;
    client
        .pick_color(webview.label(), x, y)
        .await
        .map(RgbaDto::from)
        .map_err(map_error)
}
