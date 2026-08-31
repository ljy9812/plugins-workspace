// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

/**
 * Read and write to the system clipboard.
 *
 * @module
 */

import { invoke } from '@tauri-apps/api/core'
import { Image } from '@tauri-apps/api/image'

/**
 * Writes plain text to the clipboard.
 * @example
 * ```typescript
 * import { writeText, readText } from '@tauri-apps/plugin-clipboard-manager';
 * await writeText('Tauri is awesome!');
 * assert(await readText(), 'Tauri is awesome!');
 * ```
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 */
async function writeText(
  text: string,
  opts?: { label?: string }
): Promise<void> {
  await invoke('plugin:clipboard-manager|write_text', {
    label: opts?.label,
    text
  })
}

/**
 * Gets the clipboard content as plain text.
 * @example
 * ```typescript
 * import { readText } from '@tauri-apps/plugin-clipboard-manager';
 * const clipboardText = await readText();
 * ```
 * @since 2.0.0
 */
async function readText(): Promise<string> {
  return await invoke('plugin:clipboard-manager|read_text')
}

/**
 * Writes image buffer to the clipboard.
 *
 * #### Platform-specific
 *
 * - **Android / iOS:** Not supported.
 * - **HarmonyOS (OHOS):** Supported via ArkTS PixelMap bridge.
 *
 * @example
 * ```typescript
 * import { writeImage } from '@tauri-apps/plugin-clipboard-manager';
 * const buffer = [
 *   // A red pixel
 *   255, 0, 0, 255,
 *
 *  // A green pixel
 *   0, 255, 0, 255,
 * ];
 * await writeImage(buffer);
 * ```
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 */
async function writeImage(
  image: string | Image | Uint8Array | ArrayBuffer | number[]
): Promise<void> {
  // Inline duck-type transformation instead of using transformImage().
  // Vite/Rolldown may bundle @tauri-apps/api/image into multiple chunks with
  // separate Image class definitions. When clipboard-manager's chunk has its
  // own Image class, transformImage's instanceof check fails for Image objects
  // created in a different chunk. Duck-typing (typeof .rid === 'number')
  // avoids this cross-chunk class identity problem and correctly extracts the
  // resource ID for Image instances regardless of bundling layout.
  interface RidHolder { rid: number }
  const transformed: string | number | Image | Uint8Array | ArrayBuffer | number[] | null =
    image == null
      ? null
      : typeof image === 'string'
        ? image
        : typeof (image as RidHolder).rid === 'number'
          ? (image as RidHolder).rid
          : image
  await invoke('plugin:clipboard-manager|write_image', {
    image: transformed
  })
}

/**
 * Gets the clipboard content as Uint8Array image.
 *
 * #### Platform-specific
 *
 * - **Android / iOS:** Not supported.
 * - **HarmonyOS (OHOS):** Not supported (READ_PASTEBOARD permission restriction).
 *
 * @example
 * ```typescript
 * import { readImage } from '@tauri-apps/plugin-clipboard-manager';
 *
 * const clipboardImage = await readImage();
 * const blob = new Blob([await clipboardImage.rgba()], { type: 'image' })
 * const url = URL.createObjectURL(blob)
 * ```
 * @since 2.0.0
 */
async function readImage(): Promise<Image> {
  return await invoke<number>('plugin:clipboard-manager|read_image').then(
    (rid) => new Image(rid)
  )
}

/**
 * * Writes HTML or fallbacks to write provided plain text to the clipboard.
 *
 * #### Platform-specific
 *
 * - **Android / iOS:** Not supported.
 * - **HarmonyOS (OHOS):** Supported via pasteboard HTML MIME type. The `altText` parameter is ignored (OHOS clipboard does not support alternate representations).
 *
 * @example
 * ```typescript
 * import { writeHtml } from '@tauri-apps/plugin-clipboard-manager';
 * await writeHtml('<h1>Tauri is awesome!</h1>', 'plaintext');
 * // The following will write "<h1>Tauri is awesome</h1>" as plain text
 * await writeHtml('<h1>Tauri is awesome!</h1>', '<h1>Tauri is awesome</h1>');
 * // we can read html data only as a string so there's just readText(), no readHtml()
 * assert(await readText(), '<h1>Tauri is awesome!</h1>');
 * ```
 *
 * @returns A promise indicating the success or failure of the operation.
 *
 * @since 2.0.0
 */
async function writeHtml(html: string, altText?: string): Promise<void> {
  await invoke('plugin:clipboard-manager|write_html', {
    html,
    altText
  })
}

/**
 * Clears the clipboard.
 *
 * #### Platform-specific
 *
 * - **Android:** Only supported on SDK 28+. For older releases we write an empty string to the clipboard instead.
 * - **HarmonyOS (OHOS):** Supported via `systemPasteboard.clearData()`.
 *
 * @example
 * ```typescript
 * import { clear } from '@tauri-apps/plugin-clipboard-manager';
 * await clear();
 * ```
 * @since 2.0.0
 */
async function clear(): Promise<void> {
  await invoke('plugin:clipboard-manager|clear')
}

export { writeText, readText, writeHtml, clear, readImage, writeImage }
