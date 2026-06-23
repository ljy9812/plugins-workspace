// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(any(mobile, target_env = "ohos"))]
use serde::{Deserialize, Serialize};
use tauri::{command, plugin::PermissionState, AppHandle, Runtime, State};

use crate::{Notification, NotificationData, Result};

#[command]
pub(crate) async fn is_permission_granted<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
) -> Result<Option<bool>> {
    let state = notification.permission_state()?;
    match state {
        PermissionState::Granted => Ok(Some(true)),
        PermissionState::Denied => Ok(Some(false)),
        PermissionState::Prompt | PermissionState::PromptWithRationale => Ok(None),
    }
}

#[command]
pub(crate) async fn request_permission<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
) -> Result<PermissionState> {
    notification.request_permission()
}

#[command]
pub(crate) async fn notify<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
    options: NotificationData,
) -> Result<()> {
    let mut builder = notification.builder();
    builder.data = options;
    builder.show()
}

/// Cancel pending notifications by ID, or all if none specified.
///
/// **OHOS note**: Scheduling is not supported on OHOS, so pending notifications
/// are always empty. This command effectively does nothing on OHOS.
#[cfg(any(mobile, target_env = "ohos"))]
#[command]
pub(crate) async fn cancel<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
    notifications: Option<Vec<i32>>,
) -> Result<()> {
    match notifications {
        Some(ids) if !ids.is_empty() => notification.cancel(ids),
        _ => notification.cancel_all(),
    }
}

/// Get pending notifications.
///
/// **OHOS note**: Scheduling is not supported on OHOS, so this always returns
/// an empty list. Included for API compatibility.
#[cfg(any(mobile, target_env = "ohos"))]
#[command]
pub(crate) async fn get_pending<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
) -> Result<Vec<crate::PendingNotification>> {
    notification.pending()
}

#[cfg(any(mobile, target_env = "ohos"))]
#[command]
pub(crate) async fn remove_active<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
    notifications: Option<Vec<RemoveActiveId>>,
) -> Result<()> {
    match notifications {
        Some(items) if !items.is_empty() => {
            notification.remove_active(items.into_iter().map(|i| i.id).collect())
        }
        _ => notification.remove_all_active(),
    }
}

#[cfg(any(mobile, target_env = "ohos"))]
#[command]
pub(crate) async fn get_active<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
) -> Result<Vec<crate::ActiveNotification>> {
    notification.active()
}

#[cfg(any(mobile, target_env = "ohos"))]
#[command]
pub(crate) async fn check_permissions<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
) -> Result<PermissionState> {
    notification.permission_state()
}

#[cfg(any(mobile, target_env = "ohos"))]
#[command]
pub(crate) async fn show<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
    options: NotificationData,
) -> Result<i32> {
    let mut builder = notification.builder();
    let id = options.id;
    builder.data = options;
    builder.show()?;
    Ok(id)
}

/// Result of a batch notification send.
/// Always returned as `Ok` — partial failures are captured in `failures`
/// rather than causing the entire batch to error.
#[cfg(any(mobile, target_env = "ohos"))]
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchResult {
    /// IDs of successfully sent notifications.
    pub successes: Vec<i32>,
    /// Per-notification failures: (id, error_message).
    pub failures: Vec<(i32, String)>,
}

#[cfg(any(mobile, target_env = "ohos"))]
#[command]
pub(crate) async fn batch<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
    notifications: Vec<NotificationData>,
) -> Result<BatchResult> {
    let mut successes = Vec::with_capacity(notifications.len());
    let mut failures: Vec<(i32, String)> = Vec::new();
    for data in notifications {
        let mut builder = notification.builder();
        let id = data.id;
        builder.data = data;
        match builder.show() {
            Ok(()) => successes.push(id),
            Err(e) => failures.push((id, e.to_string())),
        }
    }
    Ok(BatchResult {
        successes,
        failures,
    })
}

#[cfg(any(mobile, target_env = "ohos"))]
#[command]
pub(crate) async fn register_action_types<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
    types: Vec<crate::ActionType>,
) -> Result<()> {
    notification.register_action_types(types)
}

/// Creates a notification channel from a JSON value.
///
/// Uses `serde_json::Value` instead of `Channel` to avoid ACL camelCase/snake_case
/// mismatch: JS invoke sends `"data"` key, but `Channel` struct fields use
/// `serde(rename_all = "camelCase")` which conflicts with Tauri's command parameter
/// name resolution.
///
/// # Expected JSON schema
///
/// ```json
/// {
///   "id": "channel-id",           // string (required)
///   "name": "Channel Name",       // string (required)
///   "description": "...",         // string | null (optional)
///   "sound": "sound-name",        // string | null (optional)
///   "lights": true,               // bool (optional, default: false)
///   "lightColor": "#RRGGBB",      // string | null (optional)
///   "vibration": true,            // bool (optional, default: false)
///   "importance": 3,              // u8: 0=None, 1=Min, 2=Low, 3=Default, 4=High (optional, default: 3)
///   "visibility": 0               // i8: -1=Secret, 0=Private, 1=Public | null (optional)
/// }
/// ```
#[cfg(any(target_os = "android", target_env = "ohos"))]
#[command]
pub(crate) async fn create_channel<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
    data: serde_json::Value,
) -> Result<()> {
    let ch: crate::Channel = serde_json::from_value(data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    notification.create_channel(ch)
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
#[command]
pub(crate) async fn delete_channel<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
    id: String,
) -> Result<()> {
    notification.delete_channel(id)
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
#[command]
pub(crate) async fn list_channels<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
) -> Result<serde_json::Value> {
    notification.list_channels_raw()
}

#[cfg(any(mobile, target_env = "ohos"))]
#[derive(serde::Deserialize)]
pub(crate) struct RemoveActiveId {
    pub id: i32,
}
