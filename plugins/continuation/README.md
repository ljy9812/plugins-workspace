# @tauri-apps/plugin-continuation

Passive app-continuation restore queries for Tauri applications on OpenHarmony.

> OpenHarmony-only. On other platforms the commands reject with `unsupported`.
> Active migration (source device initiating a hand-off) is system-UI-exclusive on
> OpenHarmony and is not covered by this plugin.

## Install

**Rust** (OpenHarmony target only):

```toml
[target.'cfg(target_env = "ohos")'.dependencies]
tauri-plugin-continuation = { path = "..." }
```

```rust
#[cfg(target_env = "ohos")]
tauri::Builder::default().plugin(tauri_plugin_continuation::init())
```

**JavaScript**:

```sh
npm add @tauri-apps/plugin-continuation
```

## API

```ts
import { isContinuationRestoreLaunch, getContinuationData } from '@tauri-apps/plugin-continuation'

// Was this launch a continuation restore? Peek semantics — idempotent.
if (await isContinuationRestoreLaunch()) {
  // The wantParam the source device saved in its onContinue handler, as a raw
  // JSON string. Consuming API — only the FIRST call returns the payload.
  const payload = await getContinuationData()
  const { scrollOffset } = JSON.parse(payload!)
}
```

The payload schema is an application-level contract (key/value pairs your app
writes on the source device); the plugin passes it through verbatim.

## Permissions

`continuation:default` grants `allow-is-continuation-restore` and
`allow-get-continuation-data`. No `module.json5` permission declarations are
required — the continuation signal is captured from the ability lifecycle, not a
system service.

## License

MIT OR Apache-2.0
