![plugin-notification](https://github.com/tauri-apps/plugins-workspace/raw/v2/plugins/notification/banner.png)

Send message notifications (brief auto-expiring OS window element) to your user. Can also be used with the Notification Web API.

| Platform | Supported |
| -------- | --------- |
| Linux    | ✓         |
| Windows  | ✓         |
| macOS    | ✓         |
| Android  | ✓         |
| iOS      | ✓         |
| OpenHarmony | ✓ (scheduled notifications need the AGC agent-reminder entitlement) |

## Install

_This plugin requires a Rust version of at least **1.77.2**_

There are three general methods of installation that we can recommend.

1. Use crates.io and npm (easiest, and requires you to trust that our publishing pipeline worked)
2. Pull sources directly from Github using git tags / revision hashes (most secure)
3. Git submodule install this repo in your tauri project and then use file protocol to ingest the source (most secure, but inconvenient to use)

Install the Core plugin by adding the following to your `Cargo.toml` file:

`src-tauri/Cargo.toml`

```toml
[dependencies]
tauri-plugin-notification = "2.0.0"
# alternatively with Git:
tauri-plugin-notification = { git = "https://github.com/tauri-apps/plugins-workspace", branch = "v2" }
```

You can install the JavaScript Guest bindings using your preferred JavaScript package manager:

```sh
pnpm add @tauri-apps/plugin-notification
# or
npm add @tauri-apps/plugin-notification
# or
yarn add @tauri-apps/plugin-notification
```

## Usage

First you need to register the core plugin with Tauri:

`src-tauri/src/lib.rs`

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Then you need to add the permissions to your capabilities file:

`src-tauri/capabilities/main.json`

```json
{
  ...
  "permissions": [
    ...
    "notification:default"
  ],
  ...
}
```

Afterwards all the plugin's APIs are available through the JavaScript guest bindings:

```javascript
import {
  isPermissionGranted,
  requestPermission,
  sendNotification
} from '@tauri-apps/plugin-notification'

async function checkPermission() {
  if (!(await isPermissionGranted())) {
    return (await requestPermission()) === 'granted'
  }
  return true
}

export async function enqueueNotification(title, body) {
  if (!(await checkPermission())) {
    return
  }
  sendNotification({ title, body })
}
```

### Notification with Sound

You can add sound to your notifications on all platforms (desktop and mobile):

```javascript
import { sendNotification } from '@tauri-apps/plugin-notification'
import { platform } from '@tauri-apps/api/os'

// Basic notification with sound
sendNotification({
  title: 'New Message',
  body: 'You have a new message',
  sound: 'notification.wav' // Path to sound file
})

// Platform-specific sounds
async function sendPlatformSpecificNotification() {
  const platformName = platform()

  let soundPath
  if (platformName === 'darwin') {
    // On macOS: use system sounds or sound files in the app bundle
    soundPath = 'Ping' // macOS system sound
  } else if (platformName === 'linux') {
    // On Linux: use XDG theme sounds or file paths
    soundPath = 'message-new-instant' // XDG theme sound
  } else {
    // On Windows: use file paths
    soundPath = 'notification.wav'
  }

  sendNotification({
    title: 'Platform-specific Notification',
    body: 'This notification uses platform-specific sound',
    sound: soundPath
  })
}
```

## Platform-specific notes (OpenHarmony)

HarmonyOS exposes a subset of the notification features known from Android/iOS.
The table below documents, per `sendNotification` option, what happens on OHOS
(field references cite the SDK d.ts files shipped with DevEco Studio).

| Option | OHOS behavior |
| ------ | ------------- |
| `icon` | **Mapped** to the system notification small icon (`NotificationRequest.smallIcon`, `image.PixelMap`, notificationRequest.d.ts `smallIcon`, @since 7). The value is resolved as a rawfile resource name first (app `resources/rawfile` directory), then as a file path or `file://` URI. System limit: the icon's pixel bytes must not exceed 192 KB — the system would silently ignore oversized icons, so the plugin warns and drops them itself. Sources that fail to resolve or decode are also dropped with a warning; the notification still fires without the icon. **Immediate-publish path only** — a `schedule` that defers drops this field (see the note below the table). |
| `largeIcon` | **Mapped** to `NotificationRequest.largeIcon` (`image.PixelMap`, notificationRequest.d.ts `largeIcon`, @since 7). Same resolution order and same 192 KB pixel-bytes limit as `icon`. **Immediate-publish path only** — a `schedule` that defers drops this field (see the note below the table). |
| `attachments` | **Partially mapped**: attachments are tried in order and the first one that decodes switches the notification to the system picture layout (`NotificationContent.picture` → `NOTIFICATION_CONTENT_PICTURE`, notificationContent.d.ts; picture ≤ 2 MB pixel bytes) — an attachment that fails to resolve or decode, or exceeds the limit, is skipped and the next one is tried; after a success the later ones are ignored. Note the consequences: the picture layout **replaces** the basic/long-text/multiline layout, the picture is only shown when the notification is expanded, and `briefText`/`expandedTitle` (required by the SDK) are synthesized from the notification's own title/body/summary. Attachment URLs are resolved as file paths or `file://` URIs (the `asset` protocol is not readable from the notification plugin); if no attachment decodes the notification keeps its regular layout. **Immediate-publish path only** — a `schedule` that defers drops this field (see the note below the table). |
| `iconColor` | **Not mapped.** The system request has a `color` field, but its SDK documentation states *"Background color of the notification. Not supported currently"* (notificationRequest.d.ts `color`). Forwarding the value would silently do nothing, so it is not sent. |
| `groupSummary` | **Not mapped.** The group-summary concept is Android-only; `NotificationRequest` exposes no group-summary field (only `groupName`, which `group` maps to). |
| `silent` | **Not mapped.** Per-notification muting has no system equivalent: notification slots are created by type only (`addSlot` accepts just a `SlotType`) and slot configuration is read-only, so sound behavior cannot be silenced per notification. |

**Scheduled notifications drop `icon` / `largeIcon` / `attachments`.** The
image-field mappings above apply on the immediate-publish path only: a
`sendNotification` call with no `schedule`, or a one-shot `at` schedule whose
date is already due (also published immediately). Every other `schedule`
defers through the system reminder agent (`reminderAgentManager`), whose
`ReminderRequest` carries only title / content / notificationId / slotType —
`icon`, `largeIcon`, and `attachments` are silently dropped there.

Read-back note: `active()` reports live system notifications. The fields it
populates are listed in the `active()` API documentation. Icon images
(`smallIcon`/`largeIcon`) and picture attachments are not echoed — they exist
on the system request as `image.PixelMap`, which cannot be serialized back to
JavaScript, so `attachments` stays empty rather than being faked.

### Internal commands on OHOS

Three commands of this plugin (`batch`, `show`, `checkPermissions`) are
reachable on OHOS only through the mobile fallback channel instead of the
Rust `invoke_handler`, and are kept as internal commands deliberately.
Reachability chain, verified end-to-end:

1. **ACL** — `notification:default` grants `allow-batch` /
   `allow-check-permissions` (`permissions/default.toml`); apps using the
   default permission set (e.g. the api example's `run-app.json`) allow-list
   the commands.
2. **Rust** — the plugin's Rust `invoke_handler` (`src/lib.rs`)
   registers only the typed commands; a bare guest
   `invoke('plugin:notification|batch')` is not handled there, so tauri's
   mobile fallback takes over (`crates/tauri/src/plugin/mobile.rs`:
   `run_mobile_channel_fallback` → `run_command` → `dispatch_run_command`).
3. **ArkTS** — the fallback reaches the plugin's ArkTS command registry
   (`Plugin.ets` `getCommands()`, e.g. `commands.set('batch', ...)`), whose
   `handleBatch`/`publishBatch` implementation really publishes each
   notification and resolves the id list.

They are not part of the supported guest-API surface (no
`@tauri-apps/plugin-notification` export); the guest bindings use the
typed Rust commands. Retention differs per command:

- `show` and `checkPermissions` have internal callers: the Rust mobile
  bridge (`src/mobile.rs`) invokes them. `permission_state()` — the plugin's
  own permission checks, e.g. the `is_permission_granted` command — calls
  `checkPermissions`. `NotificationBuilder::show()` calls `show`, reached
  from the typed `notify` command; the web-notification polyfill reaches it
  through that same path indirectly (`plugin:notification|notify` → Rust
  `notify` → `show`). Removing either would break those callers.
- `batch` has no caller anywhere in this repository. It is kept because the
  `allow-batch` ACL entry is part of `notification:default` and the mobile
  fallback channel above makes it reachable; removing it would change the
  plugin's permission/behavior surface rather than remove dead code.

This note documents that they are internal, not public API.

## Contributing

PRs accepted. Please make sure to read the Contributing Guide before making a pull request.

## Partners

<table>
  <tbody>
    <tr>
      <td align="center" valign="middle">
        <a href="https://crabnebula.dev" target="_blank">
          <img src="https://github.com/tauri-apps/plugins-workspace/raw/v2/.github/sponsors/crabnebula.svg" alt="CrabNebula" width="283">
        </a>
      </td>
    </tr>
  </tbody>
</table>

For the complete list of sponsors please visit our [website](https://tauri.app#sponsors) and [Open Collective](https://opencollective.com/tauri).

## License

Code: (c) 2015 - Present - The Tauri Programme within The Commons Conservancy.

MIT or MIT/Apache 2.0 where applicable.
