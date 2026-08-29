// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::{ser::Serializer, Serialize};

/// Plugin error.
///
/// On OHOS the bridge propagates failures as `napi_ohos::Error` whose `reason` carries
/// the facade-mapped screenshot error; this maps the classified reason text back to a
/// stable error string for the frontend ("unknown webview" / "snapshot unavailable" /
/// the original message).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported")]
    Unsupported,
    #[error("{0}")]
    Screenshot(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl Error {
    /// Maps a classified facade error message to a stable frontend-facing string.
    ///
    /// The facade `ScreenshotError` Display prefixes its variants ("unknown webview: ...",
    /// "webview snapshot unavailable: ..."), so the prefix is the classification signal.
    pub fn from_screenshot_message(message: &str) -> Self {
        let lower = message.to_ascii_lowercase();
        if lower.starts_with("unknown webview") {
            Error::Screenshot("unknown webview".to_string())
        } else if lower.starts_with("webview snapshot unavailable") {
            Error::Screenshot("snapshot unavailable".to_string())
        } else {
            Error::Screenshot(message.to_string())
        }
    }
}
