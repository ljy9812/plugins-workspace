// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

import { invoke } from '@tauri-apps/api/core'

/** A webview screenshot: base64-encoded PNG data plus its pixel dimensions. */
export interface CapturedImage {
  /** Base64-encoded PNG image data. */
  pngBase64: string
  /** Snapshot width in pixels. */
  width: number
  /** Snapshot height in pixels. */
  height: number
}

/** A single pixel's color channels (0-255 each). */
export interface Rgba {
  r: number
  g: number
  b: number
  a: number
}

/**
 * Captures the calling webview as a base64 PNG with its pixel dimensions.
 *
 * The backend identifies the webview automatically (no label argument); the snapshot
 * covers the whole webview viewport. OpenHarmony only; other platforms reject
 * with `unsupported`.
 */
export async function captureWebview(): Promise<CapturedImage> {
  return await invoke<CapturedImage>('plugin:screenshot|capture_webview')
}

/**
 * Reads the color of the pixel at snapshot coordinates (`x`, `y`) of the calling
 * webview.
 *
 * Coordinates use the snapshot's pixel coordinate system (the same one as the
 * dimensions returned by `captureWebview`); out-of-bounds coordinates reject
 * with a structured error.
 */
export async function pickColorAt(x: number, y: number): Promise<Rgba> {
  return await invoke<Rgba>('plugin:screenshot|pick_color', { x, y })
}
