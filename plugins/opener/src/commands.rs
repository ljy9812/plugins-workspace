// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

use tauri::{
    ipc::{CommandScope, GlobalScope},
    AppHandle, Runtime,
};

use crate::{scope::Scope, Error, OpenerExt};

#[tauri::command]
pub async fn open_url<R: Runtime>(
    app: AppHandle<R>,
    command_scope: CommandScope<crate::scope::Entry>,
    global_scope: GlobalScope<crate::scope::Entry>,
    url: String,
    with: Option<String>,
) -> crate::Result<()> {
    let allowed = {
        let scope = Scope::new(
            &app,
            command_scope
                .allows()
                .iter()
                .chain(global_scope.allows())
                .collect(),
            command_scope
                .denies()
                .iter()
                .chain(global_scope.denies())
                .collect(),
        );
        scope.is_url_allowed(&url, with.as_deref())
    };
    if !allowed {
        return Err(Error::ForbiddenUrl { url, with });
    }

    #[cfg(target_env = "ohos")]
    {
        let _ = with; // 'open with' ignored on OHOS
        openharmony_ability::open_with_system(url)
            .await
            .map_err(|e| crate::Error::OpenharmonyAbility(e.to_string()))?;
        return Ok(());
    }
    #[cfg(not(target_env = "ohos"))]
    {
        app.opener().open_url(url, with)
    }
}

#[tauri::command]
pub async fn open_path<R: Runtime>(
    app: AppHandle<R>,
    command_scope: CommandScope<crate::scope::Entry>,
    global_scope: GlobalScope<crate::scope::Entry>,
    path: String,
    with: Option<String>,
) -> crate::Result<()> {
    let allowed = {
        let scope = Scope::new(
            &app,
            command_scope
                .allows()
                .iter()
                .chain(global_scope.allows())
                .collect(),
            command_scope
                .denies()
                .iter()
                .chain(global_scope.denies())
                .collect(),
        );
        scope.is_path_allowed(Path::new(&path), with.as_deref())?
    };
    if !allowed {
        return Err(Error::ForbiddenPath { path, with });
    }

    #[cfg(target_env = "ohos")]
    {
        let _ = with;
        // Canonicalize so relative paths (and any symlink/sandbox redirect) resolve
        // to an absolute file:// URI — url::Url::from_file_path rejects relative
        // paths. Matches the reveal_item_in_dir OHOS branch behavior.
        let canon = std::fs::canonicalize(&path)?;
        let uri = url::Url::from_file_path(&canon)
            .map_err(|_| crate::Error::InvalidPath(path.clone()))?;
        openharmony_ability::open_with_system(uri.to_string())
            .await
            .map_err(|e| crate::Error::OpenharmonyAbility(e.to_string()))?;
        return Ok(());
    }
    #[cfg(not(target_env = "ohos"))]
    {
        app.opener().open_path(path, with)
    }
}

/// TODO: in the next major version, rename to `reveal_items_in_dir`
#[tauri::command]
pub async fn reveal_item_in_dir(paths: Vec<PathBuf>) -> crate::Result<()> {
    #[cfg(target_env = "ohos")]
    {
        // OHOS has no multi-file "reveal/select" API — startAbility(viewData) on a
        // directory URI opens a single chooser. Only the first path's parent is
        // revealed; additional paths are ignored (documented limitation vs the
        // non-OHOS crate::reveal_items_in_dir which handles all paths).
        if let Some(path) = paths.first() {
            let path = std::fs::canonicalize(path)?;
            let parent = path
                .parent()
                .ok_or_else(|| crate::Error::NoParent(path.to_path_buf()))?;
            let uri = url::Url::from_file_path(parent)
                .map_err(|_| crate::Error::InvalidPath(parent.to_string_lossy().to_string()))?;
            openharmony_ability::reveal_in_dir(uri.to_string())
                .await
                .map_err(|e| crate::Error::OpenharmonyAbility(e.to_string()))?;
            return Ok(());
        }
        return Ok(());
    }
    #[cfg(not(target_env = "ohos"))]
    {
        crate::reveal_items_in_dir(&paths)
    }
}
