# @tauri-apps/plugin-accessibility

Minimal accessibility API for Tauri applications on OpenHarmony.

- `getFontScale()` — system font scale (no permission required)
- `isScreenReaderEnabled()` — screen reader state (query may require the system-level
  `ohos.permission.ACCESSIBILITY` permission; a denial rejects with an error)
- `isTouchExploreEnabled()` — touch exploration (touch guide) state
- `onAccessibilityStateChange(handler)` — screen reader state changes

Other platforms compile to `unsupported` stubs. Web content accessibility is handled
by ArkWeb's built-in ARIA support and is out of scope.
