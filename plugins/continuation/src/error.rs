// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::{ser::Serializer, Serialize};

/// Plugin error.
///
/// `set_continuation_data` validates the wantParam size budget (96 KiB, leaving
/// headroom below the ~100 KiB continuation payload limit); the read commands
/// are pure synchronous reads of Rust-side storage with no runtime failure mode
/// (mutex poisoning already degrades to `false` / empty in the facade), so their
/// only failure mode is platform support.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported")]
    Unsupported,
    #[error("payload too large: continuation data exceeds 96 KiB wantParam budget")]
    PayloadTooLarge,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
