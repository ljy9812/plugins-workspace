// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

/// Huawei account info returned to the frontend.
///
/// Mirrors `openharmony_ability::AccountInfo` (p1) field-for-field so the plugin's
/// public API stays cross-platform consistent (the desktop stub path does not see
/// openharmony-ability). On OHOS it is converted via `From`. Per p1 design D9
/// (option A), the login flow only populates `open_id`/`union_id`/`authorization_code`;
/// `uid`/`display_name`/`avatar_uri` are empty and `access_token` is `None`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub uid: String,
    pub open_id: String,
    pub union_id: String,
    pub display_name: String,
    pub avatar_uri: String,
    pub authorization_code: String,
    #[serde(default)]
    pub access_token: Option<String>,
}

#[cfg(target_env = "ohos")]
impl From<openharmony_ability::AccountInfo> for AccountInfo {
    fn from(a: openharmony_ability::AccountInfo) -> Self {
        AccountInfo {
            uid: a.uid,
            open_id: a.open_id,
            union_id: a.union_id,
            display_name: a.display_name,
            avatar_uri: a.avatar_uri,
            authorization_code: a.authorization_code,
            access_token: a.access_token,
        }
    }
}
