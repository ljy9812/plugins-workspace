// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::{ser::Serializer, Serialize};

/// Plugin error. Classified from the openharmony-ability napi error reason so the
/// frontend can branch (e.g. `NotLoggedIn` → fall back to interactive `login`).
///
/// The bridge (p1) propagates errors as `napi_ohos::Error` with reason
/// `"rejected: Error: <code>:<message>"` (the `"Error: "` prefix comes from JS
/// `Error.toString()`; p1 account.ets throws `new Error("<code>:<message>")`).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported")]
    Unsupported,
    #[error("cancelled")]
    Cancelled,
    #[error("not-logged-in")]
    NotLoggedIn,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl Error {
    /// Classify a `napi_ohos::Error` reason from openharmony-ability's account
    /// bridge into a plugin `Error`.
    ///
    /// Known codes (p1 account.ets):
    /// - `1001500001` → `Unsupported` (Account Kit not available)
    /// - `1001502001` → `NotLoggedIn` (device not signed in / not authorized)
    /// - `1001502012` → `Cancelled` (user canceled login, `ERROR_CODE_USER_CANCEL`; p3 D3)
    /// - anything else → `Other` (preserves original `code:message`)
    pub fn from_napi_reason(reason: &str) -> Self {
        // Strip the wrappers added along the bridge: "rejected: " (Rust .catch)
        // and "Error: " (JS Error.toString()).
        let rest = reason.strip_prefix("rejected: ").unwrap_or(reason);
        let rest = rest.strip_prefix("Error: ").unwrap_or(rest);

        let (code, msg) = match rest.find(':') {
            Some(i) => (rest[..i].trim(), rest[i + 1..].trim()),
            None => (rest.trim(), ""),
        };

        match code {
            "1001500001" => Error::Unsupported,
            "1001502001" => Error::NotLoggedIn,
            "1001502012" => Error::Cancelled,
            _ => {
                if msg.is_empty() {
                    Error::Other(code.to_string())
                } else {
                    Error::Other(format!("{}:{}", code, msg))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_not_logged_in() {
        assert!(matches!(
            Error::from_napi_reason("rejected: Error: 1001502001:User not signed in"),
            Error::NotLoggedIn
        ));
    }

    #[test]
    fn parse_unsupported() {
        assert!(matches!(
            Error::from_napi_reason("rejected: Error: 1001500001:Account Kit not available"),
            Error::Unsupported
        ));
    }

    #[test]
    fn parse_other_preserves_code_msg() {
        let e = Error::from_napi_reason("rejected: Error: 1001509999:some failure");
        assert!(matches!(e, Error::Other(ref s) if s == "1001509999:some failure"));
    }

    #[test]
    fn parse_cancelled() {
        assert!(matches!(
            Error::from_napi_reason("rejected: Error: 1001502012:User canceled"),
            Error::Cancelled
        ));
    }

    #[test]
    fn parse_bare_reason() {
        // Reason without the bridge wrappers.
        let e = Error::from_napi_reason("1001502001:not signed in");
        assert!(matches!(e, Error::NotLoggedIn));
    }
}
