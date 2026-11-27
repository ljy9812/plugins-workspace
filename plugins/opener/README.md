![opener](https://github.com/tauri-apps/plugins-workspace/raw/v2/plugins/opener/banner.png)

<!-- description -->

| Platform | Supported |
| -------- | --------- |
| Linux    | ✓         |
| Windows  | ✓         |
| macOS    | ✓         |
| Android  | ✓         |
| iOS      | ✓         |
| OpenHarmony | ✓ (reveal is limited, see below) |

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
tauri-plugin-opener = "2.0.0"
# alternatively with Git:
tauri-plugin-opener = { git = "https://github.com/tauri-apps/plugins-workspace", branch = "v2" }
```

You can install the JavaScript Guest bindings using your preferred JavaScript package manager:

<!-- Add the branch for installations using git! -->

```sh
pnpm add @tauri-apps/plugin-opener
# or
npm add @tauri-apps/plugin-opener
# or
yarn add @tauri-apps/plugin-opener
```

## Usage

First you need to register the core plugin with Tauri:

`src-tauri/src/lib.rs`

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Afterwards all the plugin's APIs are available through the JavaScript guest bindings:

```javascript
import { openUrl, openPath, revealItemInDir } from '@tauri-apps/plugin-opener'

// Opens the URL in the default browser
await openUrl('https://example.com')
// Or with a specific browser/app
await openUrl('https://example.com', 'firefox')

// Opens the path with the system's default app
await openPath('/path/to/file')
// Or with a specific app
await openPath('/path/to/file', 'firefox')

// Reveal a path with the system's default explorer
await revealItemInDir('/path/to/file')

// Reveal multiple paths with the system's default explorer
// Note: will be renamed to `revealItemsInDir` in the next major version
await revealItemInDir(['/path/to/file', '/path/to/another/file'])
```

### Usage from Rust

You can also use those APIs from Rust:

```rust
use tauri_plugin_opener::OpenerExt;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let opener = app.opener();

            // Opens the URL in the default browser
            opener.open_url("https://example.com", None::<&str>)?;
            // Or with a specific browser/app
            opener.open_url("https://example.com", Some("firefox"))?;

            // Opens the path with the system's default app
            opener.open_path("/path/to/file", None::<&str>)?;
            // Or with a specific app
            opener.open_path("/path/to/file", Some("firefox"))?;

            // Reveal a path with the system's default explorer
            opener.reveal_item_in_dir("/path/to/file")?;

            // Reveal multiple paths with the system's default explorer
            opener.reveal_items_in_dir(["/path/to/file"])?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## OpenHarmony (OHOS) Platform Notes

On HarmonyOS, `revealItemInDir` / `reveal_item_in_dir` (and the multi-path
variants) open the parent directory with an **implicit** `startAbility` Want:
`action: "ohos.want.action.viewData"` carrying the target in
`want.parameters.fileUri` — a file-manager virtual URI
(`file://docs/storage/Users/currentUser/<virtualName>/...`, where the
public-directory prefix's top-level directory is mapped to a virtual name
inside the URI value) — plus `want.parameters.external_storage_uuid = "LOCAL"`
(the local disk id). No flags are set, and no bundle, ability, or module name
is hardcoded: the system resolves the handler for the implicit Want.
Consequences:

- The resolved handler is not guaranteed to be the built-in file manager on
  any given device. When no handler matches the Want, `startAbility` fails
  with error `16000019` ("No matching ability is found") or `16000001` ("The
  specified ability does not exist", both per `UIAbilityContext.d.ts`) and
  the plugin surfaces a clear "reveal is not supported" error to the caller
  — there is no fallback. A `16000011` failure ("The context does not
  exist") is a destroyed calling context and surfaces as a context error,
  not as "unsupported".
- Resolution boundary (2026-11-25 2in1 device round, after the 2026-11-24
  system update): when the implicit `viewData` Want matches several
  handlers, the system AppSelector may surface an application picker
  instead of going straight to the file manager; picking a non-file-manager
  handler does not reveal anything. This is the platform's resolution
  behavior and is disclosed rather than worked around by re-pinning a
  vendor package.
- Historical risk note (2026-08-28 2in1 device experiment): a bare
  `viewData` + `file://` Want **without** the parameter contract above only
  matched archive UTD types and silently opened the notepad handler instead
  of a file manager; the `parameters.fileUri` + `external_storage_uuid`
  parameter form is what routes the dispatch to the file manager.
- Only the parent directory is opened — OHOS has no API to highlight the item
  inside it. The multi-path variants reveal only the first path's parent.
- App-sandbox paths (e.g. `appCacheDir()`, anything under `/data/storage/` or
  `/data/app/`) are rejected up front with a documented platform-limitation
  error; the handler cannot open them.
- Only public directories under `/storage/media/100/local/files/<top>/...` or
  `/storage/Users/currentUser/...` can be revealed, where `<top>` maps to a
  virtual name inside the `fileUri` value (`docs`→`Documents`,
  `desktop`→`Desktop`, `download`→`Download`, `pictures`→`Images`,
  `music`/`audio`→`Music`, `videos`→`Videos`); other prefixes are rejected
  with a documented error. Only `docs`→`Documents` has been verified
  end-to-end on a 2in1 desktop; the other virtual names are unverified.

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
