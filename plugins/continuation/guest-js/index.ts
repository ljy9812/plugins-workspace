// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

import { invoke } from '@tauri-apps/api/core'

/**
 * Returns whether the current launch is an app-continuation restore
 * (OpenHarmony: the system launched the app with
 * `launchParam.launchReason === CONTINUATION` from another device).
 *
 * Peek semantics: safe to call repeatedly; does not consume the data returned
 * by {@link getContinuationData}. OpenHarmony only; other platforms reject
 * with `unsupported`.
 */
export async function isContinuationRestoreLaunch(): Promise<boolean> {
  return await invoke<boolean>('plugin:continuation|is_continuation_restore')
}

/**
 * Returns the continuation payload (the `wantParam` key/value pairs the source
 * device saved in its `onContinue` handler) as a raw JSON string, then
 * consumes it.
 *
 * Consuming API: the first call returns the payload on a continuation
 * restore; every later call returns `null`. `null` also means the launch was
 * not a continuation restore. The payload schema is an application-level
 * contract — parse it on your side.
 */
export async function getContinuationData(): Promise<string | null> {
  return await invoke<string | null>('plugin:continuation|get_continuation_data')
}

/**
 * Pre-registers the continuation payload to migrate to the other device
 * (source-side save, OpenHarmony only; other platforms reject with
 * `unsupported`).
 *
 * Call this **while running** on the source device — the system's
 * `onContinue` callback reads the snapshot synchronously when a migration is
 * initiated (it never waits for JS). Passing `''` clears the snapshot (an
 * empty snapshot makes the system refuse the migration). The snapshot is
 * overwrite-on-set and peek-on-read: a cancelled migration leaves it intact
 * for a retry.
 *
 * Round-trip contract: the snapshot is forwarded verbatim as
 * `wantParam.continuationData`; on the target device, read it with
 * {@link getContinuationData} then `JSON.parse(...).continuationData`.
 *
 * The payload must not exceed 96 KiB (wantParam budget); larger values reject
 * with `payload too large`.
 */
export async function setContinuationData(data: string): Promise<void> {
  await invoke('plugin:continuation|set_continuation_data', { data })
}
