// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

/**
 * Huawei account one-tap login for Tauri (OpenHarmony).
 *
 * @module
 */

import { invoke } from '@tauri-apps/api/core'

/** Huawei account info. On OHOS the login flow only fills openId/unionId/authorizationCode
 *  (uid/displayName/avatarUri empty, accessToken null — p1 design D9 option A). */
export interface AccountInfo {
  uid: string
  openId: string
  unionId: string
  displayName: string
  avatarUri: string
  authorizationCode: string
  accessToken: string | null
}

/** Interactive login — forces the Huawei account login UI. */
export async function login(): Promise<AccountInfo> {
  return await invoke<AccountInfo>('plugin:huawei-account|login')
}

/** Silent login — no UI; succeeds only when already logged in & authorized.
 *  Rejects with `not-logged-in` (code 1001502001) if not signed in; caller may
 *  fall back to `login()`. */
export async function silentLogin(): Promise<AccountInfo> {
  return await invoke<AccountInfo>('plugin:huawei-account|silent_login')
}

/** Logout — cancels the app's Huawei account authorization on this device. */
export async function logout(): Promise<void> {
  await invoke('plugin:huawei-account|logout')
}
