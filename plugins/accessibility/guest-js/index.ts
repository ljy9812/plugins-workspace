// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

/**
 * Returns the system font scale (default 1.0). Requires no permission.
 * OpenHarmony only; other platforms reject with `unsupported`.
 */
export async function getFontScale(): Promise<number> {
  return await invoke<number>('plugin:accessibility|get_font_scale')
}

/**
 * Returns whether a screen reader is currently open.
 *
 * OHOS documents `ohos.permission.ACCESSIBILITY` (system_core) for this query;
 * a third-party denial rejects with a structured error instead of `false`.
 */
export async function isScreenReaderEnabled(): Promise<boolean> {
  return await invoke<boolean>('plugin:accessibility|is_screen_reader_enabled')
}

/** Returns whether touch exploration (touch guide) is enabled. */
export async function isTouchExploreEnabled(): Promise<boolean> {
  return await invoke<boolean>('plugin:accessibility|is_touch_explore_enabled')
}

/**
 * Listen for screen-reader state changes.
 *
 * The native subscription is owned by the plugin (set up at init); JS listeners fan
 * out through the Tauri event system, so multiple handlers are supported and
 * unsubscribing one does not affect the others.
 */
export async function onAccessibilityStateChange(
  handler: (enabled: boolean) => void,
): Promise<UnlistenFn> {
  return await listen<boolean>('accessibility-state-changed', (event) =>
    handler(event.payload),
  )
}
