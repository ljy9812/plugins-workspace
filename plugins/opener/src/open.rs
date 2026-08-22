// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Types and functions related to shell.

use std::path::Path;
#[cfg(not(target_env = "ohos"))]
use std::ffi::OsStr;

#[cfg(not(target_env = "ohos"))]
pub(crate) fn open<P: AsRef<OsStr>, S: AsRef<str>>(path: P, with: Option<S>) -> crate::Result<()> {
    match with {
        Some(program) => ::open::with_detached(path, program.as_ref()),
        None => ::open::that_detached(path),
    }
    .map_err(Into::into)
}

/// Opens URL with the program specified in `with`, or system default if `None`.
///
/// ## Platform-specific:
///
/// - **Android / iOS**: Always opens using default program.
///
/// # Examples
///
/// ```rust,no_run
/// tauri::Builder::default()
///   .setup(|app| {
///     // open the given URL on the system default browser
///     tauri_plugin_opener::open_url("https://github.com/tauri-apps/tauri", None::<&str>)?;
///     Ok(())
///   });
/// ```
pub async fn open_url<P: AsRef<str>, S: AsRef<str>>(url: P, with: Option<S>) -> crate::Result<()> {
    let url = url.as_ref();
    #[cfg(target_env = "ohos")]
    {
        use openharmony_ability_plugin_url::UrlExt;

        // 'open with' (with-program) is unsupported on OHOS — system default only.
        let _ = with;
        let ohos_app = tauri::ohos::APP
            .lock()
            .ok()
            .and_then(|g| g.as_ref().cloned())
            .ok_or_else(|| {
                crate::Error::OpenharmonyAbility("OHOS APP not initialized".to_string())
            })?;
        ohos_app
            .open_url(url.to_string())
            .await
            .map_err(|e| crate::Error::OpenharmonyAbility(e.to_string()))?;
        return Ok(());
    }
    #[cfg(not(target_env = "ohos"))]
    {
        open(url, with)
    }
}

/// Opens path with the program specified in `with`, or system default if `None`.
///
/// ## Platform-specific:
///
/// - **Android / iOS**: Always opens using default program.
///
/// # Examples
///
/// ```rust,no_run
/// tauri::Builder::default()
///   .setup(|app| {
///     // open the given URL on the system default explorer
///     tauri_plugin_opener::open_path("/path/to/file", None::<&str>)?;
///     Ok(())
///   });
/// ```
pub async fn open_path<P: AsRef<Path>, S: AsRef<str>>(path: P, with: Option<S>) -> crate::Result<()> {
    #[cfg(target_env = "ohos")]
    {
        use openharmony_ability_plugin_url::UrlExt;

        let _ = with; // 'open with' unsupported on OHOS
        // Canonicalize so relative paths (and any symlink/sandbox redirect) resolve
        // to an absolute file:// URI — url::Url::from_file_path rejects relative
        // paths. Matches the reveal_item_in_dir OHOS branch behavior.
        let canon = std::fs::canonicalize(path.as_ref())?;
        let uri = url::Url::from_file_path(&canon)
            .map_err(|_| crate::Error::InvalidPath(path.as_ref().to_string_lossy().to_string()))?;
        let ohos_app = tauri::ohos::APP
            .lock()
            .ok()
            .and_then(|g| g.as_ref().cloned())
            .ok_or_else(|| {
                crate::Error::OpenharmonyAbility("OHOS APP not initialized".to_string())
            })?;
        ohos_app
            .open_url(uri.to_string())
            .await
            .map_err(|e| crate::Error::OpenharmonyAbility(e.to_string()))?;
        return Ok(());
    }
    #[cfg(not(target_env = "ohos"))]
    {
        let path = path.as_ref();
        if with.is_none() {
            // Returns an IO error if not exists, and besides `exists()` is a shorthand for `metadata()`
            _ = path.metadata()?;
        }
        open(path, with)
    }
}
