# @tauri-apps/plugin-huawei-account

Huawei Account one-tap login via Account Kit for Tauri OpenHarmony apps.

Returns `AccountInfo` (openId / unionId / authorizationCode; profile fields empty per design option A) for use in client identification and server-side validation.

| Platform | Supported |
| -------- | --------- |
| OHOS     | ✓ (mobile + desktop) |
| Linux    | ✗ (returns `unsupported`) |
| Windows  | ✗ (returns `unsupported`) |
| macOS    | ✗ (returns `unsupported`) |

> This plugin is OHOS-only. On other platforms every command returns `Error::Unsupported` without touching any account capability.

## Install

_This plugin requires a Rust version of at least **1.77.2**_

Install the Rust plugin by adding the following to your `Cargo.toml`:

`src-tauri/Cargo.toml`

```toml
[target.'cfg(target_env = "ohos")'.dependencies]
tauri-plugin-huawei-account = "2.0.0"
# alternatively with Git:
tauri-plugin-huawei-account = { git = "https://github.com/tauri-apps/plugins-workspace", branch = "v2" }
```

Install the JavaScript guest bindings:

```sh
pnpm add @tauri-apps/plugin-huawei-account
# or
npm add @tauri-apps/plugin-huawei-account
# or
yarn add @tauri-apps/plugin-huawei-account
```

## Usage

Register the plugin with Tauri:

`src-tauri/src/lib.rs`

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_huawei_account::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Then use the guest bindings:

```javascript
import { login, silentLogin, logout } from '@tauri-apps/plugin-huawei-account'

// Interactive login (pops Huawei one-tap login UI when not signed in)
const info = await login()
console.log(info.openId, info.unionId, info.authorizationCode)

// Silent login (no UI; succeeds only when already signed in & authorized)
const info = await silentLogin()

// Logout (cancels the app's Huawei account authorization on this device)
await logout()
```

## Prerequisites

- **AGC configuration**: Register your app at [AppGallery Connect](https://developer.huawei.com/consumer/cn/agconnect/) with bundle name matching your app, obtain the OAuth2 `client_id`, and declare it in `module.json5` metadata. Download `agconnect-services.json` into `gen/ohos/entry_*/src/main/resources/rawfile/`.
- **Signing**: Register your signing certificate SHA256 at AGC; it must match the HAP's actual signing certificate.
- **Account Kit permission**: The one-tap login capability must be granted by AGC.
- **Account Kit bridge**: Depends on `openharmony-ability` (account feature) which wraps `@kit.AccountKit`.

## Error Handling

Errors are classified and serialized to strings:

| Error | Code | Meaning |
|-------|------|---------|
| `unsupported` | 1001500001 | Device lacks Account Kit / non-OHOS platform |
| `not-logged-in` | 1001502001 | Device not signed in to Huawei account |
| `cancelled` | 1001502012 | User canceled the login UI |
| `other` | — | Other business error (preserves original code:message) |

Front-end fallback pattern: catch `not-logged-in` from `silentLogin`, then call `login` to pop the interactive UI.

## Contributing

PRs accepted. Please make sure to read the Contributing Guide before making a pull request.

## License

Code: (c) 2015 - Present - The Tauri Programme within The Commons Conservancy.

MIT or MIT/Apache 2.0 where applicable.
