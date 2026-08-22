// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(target_env = "ohos")]
use serde::Deserialize;
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
#[cfg(target_env = "ohos")]
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
#[cfg(target_env = "ohos")]
#[command]
pub(crate) async fn get_pending<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
) -> Result<Vec<crate::PendingNotification>> {
    notification.pending()
}

#[cfg(target_env = "ohos")]
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

#[cfg(target_env = "ohos")]
#[command]
pub(crate) async fn get_active<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
) -> Result<Vec<crate::ActiveNotification>> {
    notification.active()
}

#[cfg(target_env = "ohos")]
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
/// OHOS: the JS SDK sends `{ ...channel }` (spread), so the channel fields are
/// top-level keys in the invoke body (e.g. `{ "id": "...", "name": "..." }`).
/// A normal `data: serde_json::Value` parameter looks for a `"data"` key that
/// doesn't exist. `ChannelArg` bypasses the key lookup and deserializes from
/// the entire payload.
#[cfg(target_env = "ohos")]
pub(crate) struct ChannelArg(crate::Channel);

#[cfg(target_env = "ohos")]
impl<'de, R: Runtime> tauri::ipc::CommandArg<'de, R> for ChannelArg {
    fn from_command(command: tauri::ipc::CommandItem<'de, R>) -> std::result::Result<Self, tauri::ipc::InvokeError> {
        match command.message.payload() {
            tauri::ipc::InvokeBody::Json(v) => {
                let ch: crate::Channel = serde_json::from_value(v.clone())
                    .map_err(|e| {
                        tauri::ipc::InvokeError::from_error(
                            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
                        )
                    })?;
                Ok(ChannelArg(ch))
            }
            tauri::ipc::InvokeBody::Raw(_) => Err(tauri::ipc::InvokeError::from_error(
                std::io::Error::new(std::io::ErrorKind::InvalidData, "create_channel expects JSON payload"),
            )),
        }
    }
}

#[cfg(target_env = "ohos")]
#[command]
pub(crate) async fn create_channel<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
    _channel: ChannelArg,
) -> Result<()> {
    notification.create_channel(_channel.0)
}

#[cfg(target_env = "ohos")]
#[command]
pub(crate) async fn delete_channel<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
    id: String,
) -> Result<()> {
    notification.delete_channel(id)
}

#[cfg(target_env = "ohos")]
#[command]
pub(crate) async fn list_channels<R: Runtime>(
    _app: AppHandle<R>,
    notification: State<'_, Notification<R>>,
) -> Result<serde_json::Value> {
    notification.list_channels_raw()
}

#[cfg(target_env = "ohos")]
#[derive(serde::Deserialize)]
pub(crate) struct RemoveActiveId {
    pub id: i32,
}
