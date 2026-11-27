// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

/**
 * Send toast notifications (brief auto-expiring OS window element) to your user.
 * Can also be used with the Notification Web API.
 *
 * @module
 */

import {
  invoke,
  type PluginListener,
  addPluginListener
} from '@tauri-apps/api/core'

export type { PermissionState } from '@tauri-apps/api/core'

/**
 * Options to send a notification.
 *
 * @since 2.0.0
 */
interface Options {
  /**
   * The notification identifier to reference this object later. Must be a 32-bit integer.
   */
  id?: number
  /**
   * Identifier of the {@link Channel} that deliveres this notification.
   *
   * If the channel does not exist, the notification won't fire.
   * Make sure the channel exists with {@link listChannels} and {@link createChannel}.
   */
  channelId?: string
  /**
   * Notification title.
   */
  title: string
  /**
   * Optional notification body.
   * */
  body?: string
  /**
   * Schedule this notification to fire on a later time or a fixed interval.
   *
   * #### Platform-specific
   *
   * - **HarmonyOS (OHOS):** Scheduled notifications are published as
   *   `reminderAgentManager` system reminders, so they still fire when the
   *   app is backgrounded or killed. Repeating `at` schedules repeat on the
   *   scheduled day-of-month; `every` month/year schedules repeat on the
   *   creation day (months without that day are skipped; a Feb-29 yearly
   *   schedule fires only in leap years).
   *   Schedules without a system-reminder equivalent reject instead of
   *   scheduling (every hour/minute/second, every N units, twoWeeks, past
   *   repeating dates, and interval rules mixing month/day with weekday).
   *   Reminders are quota-managed: without the app's AGC agent-reminder
   *   entitlement the schedule call rejects with error 1700002, and with
   *   notifications disabled it rejects with 1700001. A one-shot `at` that
   *   is already due fires immediately.
   */
  schedule?: Schedule
  /**
   * Multiline text.
   * Changes the notification style to big text.
   * Cannot be used with `inboxLines`.
   */
  largeBody?: string
  /**
   * Detail text for the notification with `largeBody`, `inboxLines` or `groupSummary`.
   */
  summary?: string
  /**
   * Defines an action type for this notification.
   */
  actionTypeId?: string
  /**
   * Identifier used to group multiple notifications.
   *
   * https://developer.apple.com/documentation/usernotifications/unmutablenotificationcontent/1649872-threadidentifier
   *
   * On HarmonyOS (OHOS): maps to the system notification group name.
   */
  group?: string
  /**
   * Instructs the system that this notification is the summary of a group on Android.
   *
   * OHOS: not supported — the system notification request has no group-summary
   * equivalent (the concept is Android-only; the request only exposes a group
   * name, which {@link Options.group} maps to).
   */
  groupSummary?: boolean
  /**
   * The sound resource name or file path for the notification.
   *
   * Platform specific behavior:
   * - On macOS: use system sounds (e.g., "Ping", "Blow") or sound files in the app bundle
   * - On Linux: use XDG theme sounds (e.g., "message-new-instant") or file paths
   * - On Windows: use file paths to sound files (.wav format)
   * - On Mobile: use resource names
   */
  sound?: string
  /**
   * List of lines to add to the notification.
   * Changes the notification style to inbox.
   * Cannot be used with `largeBody`.
   *
   * Only supports up to 5 lines.
   */
  inboxLines?: string[]
  /**
   * Notification icon.
   *
   * On Android the icon must be placed in the app's `res/drawable` folder.
   *
   * On OHOS: maps to the system notification small icon (`smallIcon`,
   * image.PixelMap). The value is resolved as a rawfile resource name first
   * (app `resources/rawfile` directory), then as a file path or `file://`
   * URI. Sources that fail to resolve or decode, and images whose pixel
   * bytes exceed the system limit (192 KB), are dropped with a warning —
   * the notification still fires, just without the icon. Immediate-publish
   * path only: a `schedule` that defers drops this field (the system
   * reminder carries only title/body/id/slot type).
   */
  icon?: string
  /**
   * Notification large icon (Android).
   *
   * The icon must be placed in the app's `res/drawable` folder.
   *
   * On OHOS: maps to the system notification large icon (`largeIcon`,
   * image.PixelMap). Resolution and degradation behave exactly like
   * {@link Options.icon} (rawfile name first, then path/`file://` URI;
   * undecodable or oversized sources are dropped with a warning).
   * Immediate-publish path only: a `schedule` that defers drops this
   * field (the system reminder carries only title/body/id/slot type).
   */
  largeIcon?: string
  /**
   * Icon color on Android.
   *
   * OHOS: not supported — the system request has a `color` field, but its
   * SDK documentation states "Background color of the notification. Not
   * supported currently" (notificationRequest.d.ts), so the value is not
   * forwarded (mapping it would silently do nothing).
   */
  iconColor?: string
  /**
   * Notification attachments.
   *
   * On OHOS: attachments are tried in order and the first one that decodes
   * switches the notification to the system picture layout
   * (NOTIFICATION_CONTENT_PICTURE) — the picture is shown when the
   * notification is expanded, and it replaces the basic/long-text/multiline
   * layout. An attachment that fails to resolve or decode, or exceeds the
   * system limit (2 MB pixel bytes), is skipped and the next one is tried;
   * after a success the later ones are ignored. Attachment URLs are
   * resolved as file paths or `file://` URIs (the `asset` protocol is not
   * readable from the notification plugin). If no attachment decodes, the
   * notification keeps its regular layout. Immediate-publish path only: a
   * `schedule` that defers drops this field (the system reminder carries
   * only title/body/id/slot type).
   */
  attachments?: Attachment[]
  /**
   * Extra payload to store in the notification.
   *
   * On HarmonyOS (OHOS): round-trips through `active()` (stored in the
   * system notification request).
   */
  extra?: Record<string, unknown>
  /**
   * If true, the notification cannot be dismissed by the user on Android.
   *
   * An application service must manage the dismissal of the notification.
   * It is typically used to indicate a background task that is pending (e.g. a file download)
   * or the user is engaged with (e.g. playing music).
   *
   * On HarmonyOS (OHOS): maps to the system ongoing-notification flag.
   */
  ongoing?: boolean
  /**
   * Automatically cancel the notification when the user clicks on it.
   */
  autoCancel?: boolean
  /**
   * Changes the notification presentation to be silent on iOS (no badge, no sound, not listed).
   *
   * OHOS: not supported — per-notification muting has no system equivalent.
   * Notification slots are created by type only (`addSlot` accepts just a
   * SlotType) and their configuration is read-only, so sound behavior cannot
   * be silenced per notification.
   */
  silent?: boolean
  /**
   * Notification visibility.
   */
  visibility?: Visibility
  /**
   * Sets the number of items this notification represents on Android.
   */
  number?: number
}

interface ScheduleInterval {
  year?: number
  month?: number
  day?: number
  /**
   * 1 - Sunday
   * 2 - Monday
   * 3 - Tuesday
   * 4 - Wednesday
   * 5 - Thursday
   * 6 - Friday
   * 7 - Saturday
   */
  weekday?: number
  hour?: number
  minute?: number
  second?: number
}

enum ScheduleEvery {
  Year = 'year',
  Month = 'month',
  TwoWeeks = 'twoWeeks',
  Week = 'week',
  Day = 'day',
  Hour = 'hour',
  Minute = 'minute',
  /**
   * Not supported on iOS.
   */
  Second = 'second'
}

class Schedule {
  at:
    | {
        date: Date
        repeating: boolean
        allowWhileIdle: boolean
      }
    | undefined

  interval:
    | {
        interval: ScheduleInterval
        allowWhileIdle: boolean
      }
    | undefined

  every:
    | {
        interval: ScheduleEvery
        count: number
        allowWhileIdle: boolean
      }
    | undefined

  static at(date: Date, repeating = false, allowWhileIdle = false): Schedule {
    return {
      at: { date, repeating, allowWhileIdle },
      interval: undefined,
      every: undefined
    }
  }

  static interval(
    interval: ScheduleInterval,
    allowWhileIdle = false
  ): Schedule {
    return {
      at: undefined,
      interval: { interval, allowWhileIdle },
      every: undefined
    }
  }

  static every(
    kind: ScheduleEvery,
    count: number,
    allowWhileIdle = false
  ): Schedule {
    return {
      at: undefined,
      interval: undefined,
      every: { interval: kind, count, allowWhileIdle }
    }
  }
}

/**
 * Attachment of a notification.
 */
interface Attachment {
  /** Attachment identifier. */
  id: string
  /**
   * Attachment URL. Accepts the `asset` and `file` protocols.
   *
   * On OHOS the `asset` protocol is not readable from the notification
   * plugin; attachment URLs are resolved as file paths or `file://` URIs.
   */
  url: string
}

interface Action {
  id: string
  title: string
  requiresAuthentication?: boolean
  foreground?: boolean
  destructive?: boolean
  input?: boolean
  inputButtonTitle?: string
  inputPlaceholder?: string
}

interface ActionType {
  /**
   * The identifier of this action type
   */
  id: string
  /**
   * The list of associated actions
   */
  actions: Action[]
  hiddenPreviewsBodyPlaceholder?: string
  customDismissAction?: boolean
  allowInCarPlay?: boolean
  hiddenPreviewsShowTitle?: boolean
  hiddenPreviewsShowSubtitle?: boolean
}

interface PendingNotification {
  id: number
  title?: string
  body?: string
  /**
   * May be missing on HarmonyOS for entries recovered from system-proxied
   * reminders after a process restart (one-shot countdown reminders cannot
   * restore their original date).
   */
  schedule?: Schedule
}

interface ActiveNotification {
  id: number
  tag?: string
  title?: string
  body?: string
  group?: string
  groupSummary: boolean
  data: Record<string, string>
  extra: Record<string, unknown>
  attachments: Attachment[]
  actionTypeId?: string
  schedule?: Schedule
  sound?: string
}

enum Importance {
  None = 0,
  Min,
  Low,
  Default,
  High
}

enum Visibility {
  Secret = -1,
  Private,
  Public
}

interface Channel {
  id: string
  name: string
  description?: string
  sound?: string
  lights?: boolean
  lightColor?: string
  vibration?: boolean
  importance?: Importance
  visibility?: Visibility
}

/**
 * Checks if the permission to send notifications is granted.
 * @example
 * ```typescript
 * import { isPermissionGranted } from '@tauri-apps/plugin-notification';
 * const permissionGranted = await isPermissionGranted();
 * ```
 *
 * @since 2.0.0
 */
async function isPermissionGranted(): Promise<boolean> {
  if (window.Notification.permission !== 'default') {
    return await Promise.resolve(window.Notification.permission === 'granted')
  }
  return await invoke('plugin:notification|is_permission_granted')
}

/**
 * Requests the permission to send notifications.
 * @example
 * ```typescript
 * import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification';
 * let permissionGranted = await isPermissionGranted();
 * if (!permissionGranted) {
 *   const permission = await requestPermission();
 *   permissionGranted = permission === 'granted';
 * }
 * ```
 *
 * @returns A promise resolving to whether the user granted the permission or not.
 *
 * @since 2.0.0
 */
async function requestPermission(): Promise<NotificationPermission> {
  return await window.Notification.requestPermission()
}

/**
 * Sends a notification to the user.
 * @example
 * ```typescript
 * import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';
 * let permissionGranted = await isPermissionGranted();
 * if (!permissionGranted) {
 *   const permission = await requestPermission();
 *   permissionGranted = permission === 'granted';
 * }
 * if (permissionGranted) {
 *   sendNotification('Tauri is awesome!');
 *   sendNotification({ title: 'TAURI', body: 'Tauri is awesome!' });
 * }
 * ```
 *
 * @since 2.0.0
 */
function sendNotification(options: Options | string): void {
  if (typeof options === 'string') {
    new window.Notification(options)
  } else {
    new window.Notification(options.title, options)
  }
}

/**
 * Register actions that are performed when the user clicks on the notification.
 *
 * @example
 * ```typescript
 * import { registerActionTypes } from '@tauri-apps/plugin-notification';
 * await registerActionTypes([{
 *   id: 'tauri',
 *   actions: [{
 *     id: 'my-action',
 *     title: 'Settings'
 *   }]
 * }])
 * ```
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 */
async function registerActionTypes(types: ActionType[]): Promise<void> {
  await invoke('plugin:notification|register_action_types', { types })
}

/**
 * Retrieves the list of pending notifications.
 *
 * #### Platform-specific
 *
 * - **HarmonyOS (OHOS):** System-proxied reminders outlive the app process.
 *   The list is reconciled with the system (`getAllValidReminders`), so
 *   reminders that survived a process restart are still listed, while
 *   one-shot reminders that already fired are pruned. Recovered entries may
 *   omit `schedule`.
 *
 * @example
 * ```typescript
 * import { pending } from '@tauri-apps/plugin-notification';
 * const pendingNotifications = await pending();
 * ```
 *
 * @returns A promise resolving to the list of pending notifications.
 *
 * @since 2.0.0
 */
async function pending(): Promise<PendingNotification[]> {
  return await invoke('plugin:notification|get_pending')
}

/**
 * Cancels the pending notifications with the given list of identifiers.
 *
 * #### Platform-specific
 *
 * - **HarmonyOS (OHOS):** System-proxied reminders outlive the app process.
 *   Cancelling by id still works after a process restart (reconciled with the
 *   system via `getAllValidReminders`); `cancelAll` always clears every
 *   system-level reminder as well.
 *
 * @example
 * ```typescript
 * import { cancel } from '@tauri-apps/plugin-notification';
 * await cancel([-34234, 23432, 4311]);
 * ```
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 */
async function cancel(notifications: number[]): Promise<void> {
  await invoke('plugin:notification|cancel', { notifications })
}

/**
 * Cancels all pending notifications.
 *
 * @example
 * ```typescript
 * import { cancelAll } from '@tauri-apps/plugin-notification';
 * await cancelAll();
 * ```
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 */
async function cancelAll(): Promise<void> {
  await invoke('plugin:notification|cancel')
}

/**
 * Retrieves the list of active notifications.
 *
 * #### Platform-specific
 *
 * - **HarmonyOS (OHOS):** Sourced from the system's live list
 *   (`getActiveNotifications`), so it survives app restarts. Populated:
 *   `id`, `title`, `body` and `extra` (round-trips
 *   {@link Options.extra}). Carried through when the system reports
 *   them: `group`, `sound` and `tag` (the plugin itself never sets the
 *   system notification label). Always empty: `data`, `attachments`,
 *   `groupSummary`, `actionTypeId` and `schedule` (no system equivalent).
 *
 * @example
 * ```typescript
 * import { active } from '@tauri-apps/plugin-notification';
 * const activeNotifications = await active();
 * ```
 *
 * @returns A promise resolving to the list of active notifications.
 *
 * @since 2.0.0
 */
async function active(): Promise<ActiveNotification[]> {
  return await invoke('plugin:notification|get_active')
}

/**
 * Removes the active notifications with the given list of identifiers.
 *
 * @example
 * ```typescript
 * import { cancel } from '@tauri-apps/plugin-notification';
 * await cancel([-34234, 23432, 4311])
 * ```
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 */
async function removeActive(
  notifications: Array<{ id: number; tag?: string }>
): Promise<void> {
  await invoke('plugin:notification|remove_active', { notifications })
}

/**
 * Removes all active notifications.
 *
 * @example
 * ```typescript
 * import { removeAllActive } from '@tauri-apps/plugin-notification';
 * await removeAllActive()
 * ```
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 */
async function removeAllActive(): Promise<void> {
  await invoke('plugin:notification|remove_active')
}

/**
 * Creates a notification channel.
 *
 * #### Platform-specific
 *
 * - **HarmonyOS (OHOS):** Channels map onto the system's four notification
 *   slot types (derived from `importance`); the system keeps a single slot
 *   per type. `sound`, `lights`, `lightColor`, `vibration` and `visibility`
 *   are kept in a local table (returned by {@link channels}) but the system
 *   slot itself uses its default configuration — HarmonyOS exposes no
 *   public API to customize a slot, so channels sharing the same
 *   `importance` share one system slot (a runtime warning is logged when
 *   that happens). Per-notification {@link Options.sound} still applies.
 *
 * @example
 * ```typescript
 * import { createChannel, Importance, Visibility } from '@tauri-apps/plugin-notification';
 * await createChannel({
 *   id: 'new-messages',
 *   name: 'New Messages',
 *   lights: true,
 *   vibration: true,
 *   importance: Importance.Default,
 *   visibility: Visibility.Private
 * });
 * ```
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 */
async function createChannel(channel: Channel): Promise<void> {
  await invoke('plugin:notification|create_channel', { ...channel })
}

/**
 * Removes the channel with the given identifier.
 *
 * @example
 * ```typescript
 * import { removeChannel } from '@tauri-apps/plugin-notification';
 * await removeChannel();
 * ```
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 */
async function removeChannel(id: string): Promise<void> {
  await invoke('plugin:notification|delete_channel', { id })
}

/**
 * Retrieves the list of notification channels.
 *
 * @example
 * ```typescript
 * import { channels } from '@tauri-apps/plugin-notification';
 * const notificationChannels = await channels();
 * ```
 *
 * @returns A promise resolving to the list of notification channels.
 *
 * @since 2.0.0
 */
async function channels(): Promise<Channel[]> {
  // `list_channels` (not v2's `listChannels`) matches the snake_case used by
  // every other command of this plugin; on Android/iOS the mobile fallback
  // converts it back to camelCase, and desktop never supported channels.
  return await invoke('plugin:notification|list_channels')
}

async function onNotificationReceived(
  cb: (notification: Options) => void
): Promise<PluginListener> {
  return await addPluginListener('notification', 'notification', cb)
}

async function onAction(
  cb: (notification: Options) => void
): Promise<PluginListener> {
  return await addPluginListener('notification', 'actionPerformed', cb)
}

export type {
  Attachment,
  Options,
  Action,
  ActionType,
  PendingNotification,
  ActiveNotification,
  Channel,
  ScheduleInterval
}

export {
  Importance,
  Visibility,
  sendNotification,
  requestPermission,
  isPermissionGranted,
  registerActionTypes,
  pending,
  cancel,
  cancelAll,
  active,
  removeActive,
  removeAllActive,
  createChannel,
  removeChannel,
  channels,
  onNotificationReceived,
  onAction,
  Schedule,
  ScheduleEvery
}
