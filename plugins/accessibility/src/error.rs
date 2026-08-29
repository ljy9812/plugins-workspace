// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::{ser::Serializer, Serialize};

/// Plugin error.
///
/// On OHOS the bridge propagates failures as `napi_ohos::Error` whose `reason` carries
/// the facade-mapped accessibility error (e.g. a permission denial for the screen
/// reader query); those surface as `Error::Accessibility` without further classification
/// — the minimal API only needs the message to reach the frontend.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported")]
    Unsupported,
    #[error("{0}")]
    Accessibility(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl Error {
    pub fn from_napi_reason(reason: &str) -> Self {
        Error::Accessibility(reason.to_string())
    }
}
