# @tauri-apps/plugin-screenshot

In-app webview screenshot and color picking for Tauri applications on OpenHarmony.

> OpenHarmony-only. On other platforms the commands reject with `unsupported`.

## Install

**Rust** (OpenHarmony target only):

```toml
[target.'cfg(target_env = "ohos")'.dependencies]
tauri-plugin-screenshot = { path = "..." }
```

```rust
#[cfg(target_env = "ohos")]
tauri::Builder::default().plugin(tauri_plugin_screenshot::init())
```

**JavaScript**:

```sh
npm add @tauri-apps/plugin-screenshot
```

## API

```ts
import { captureWebview, pickColorAt } from '@tauri-apps/plugin-screenshot'

// Full-viewport snapshot of the calling webview.
const image = await captureWebview()
// image.pngBase64 — base64 PNG
// image.width / image.height — pixel dimensions

// Read one pixel (snapshot coordinate system — use the dimensions returned by
// captureWebview to scale CSS coordinates).
const { r, g, b, a } = await pickColorAt(x, y)
```

## Permissions

`screenshot:default` grants `allow-capture-webview` and `allow-pick-color`. No
`module.json5` permission declarations are required — the snapshot is taken from the
app's own webview (ArkWeb `webPageSnapshot`), not the screen.

## Errors

- `unknown webview` — the calling webview is not registered (e.g. torn down).
- `snapshot unavailable` — the snapshot could not be produced (retries exhausted,
  timeout, or the webview has not rendered its first frame).
- anything else — packing / pixel-read / coordinate failure, message included.

## License

MIT OR Apache-2.0
