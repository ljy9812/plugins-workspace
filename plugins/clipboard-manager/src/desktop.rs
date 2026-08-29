// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::de::DeserializeOwned;
use std::borrow::Cow;
use tauri::{image::Image, plugin::PluginApi, AppHandle, Runtime};

#[cfg(all(desktop, not(target_env = "ohos")))]
use std::sync::Mutex;

// ─── Desktop (non-OHOS): arboard-based clipboard ───

#[cfg(all(desktop, not(target_env = "ohos")))]
use arboard::ImageData;

#[cfg(all(desktop, not(target_env = "ohos")))]
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Clipboard<R>> {
    let clipboard_result = arboard::Clipboard::new().map(|c| Mutex::new(Some(c)));
    Ok(Clipboard {
        app: app.clone(),
        clipboard: clipboard_result,
    })
}

#[cfg(all(desktop, not(target_env = "ohos")))]
/// Access to the clipboard APIs.
pub struct Clipboard<R: Runtime> {
    #[allow(dead_code)]
    app: AppHandle<R>,
    // According to arboard docs the clipboard must be dropped before exit.
    // Since tauri doesn't call drop on exit we'll use an Option to take() on RunEvent::Exit.
    clipboard: Result<Mutex<Option<arboard::Clipboard>>, arboard::Error>,
}

#[cfg(all(desktop, not(target_env = "ohos")))]
impl<R: Runtime> Clipboard<R> {
    pub fn write_text<'a, T: Into<Cow<'a, str>>>(&self, text: T) -> crate::Result<()> {
        match &self.clipboard {
            Ok(clipboard) => clipboard
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .set_text(text)
                .map_err(Into::into),
            Err(e) => Err(crate::Error::Clipboard(e.to_string())),
        }
    }

    pub fn write_image(&self, image: &Image<'_>) -> crate::Result<()> {
        match &self.clipboard {
            Ok(clipboard) => clipboard
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .set_image(ImageData {
                    bytes: Cow::Borrowed(image.rgba()),
                    width: image.width() as usize,
                    height: image.height() as usize,
                })
                .map_err(Into::into),
            Err(e) => Err(crate::Error::Clipboard(e.to_string())),
        }
    }

    /// Warning: This method should not be used on the main thread! Otherwise the underlying libraries may deadlock on Linux, freezing the whole app, when trying to copy data copied from this app, for example if the user copies text from the WebView.
    pub fn read_text(&self) -> crate::Result<String> {
        match &self.clipboard {
            Ok(clipboard) => {
                let text = clipboard.lock().unwrap().as_mut().unwrap().get_text()?;
                Ok(text)
            }
            Err(e) => Err(crate::Error::Clipboard(e.to_string())),
        }
    }

    pub fn write_html<'a, T: Into<Cow<'a, str>>>(
        &self,
        html: T,
        alt_text: Option<T>,
    ) -> crate::Result<()> {
        match &self.clipboard {
            Ok(clipboard) => clipboard
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .set_html(html, alt_text)
                .map_err(Into::into),
            Err(e) => Err(crate::Error::Clipboard(e.to_string())),
        }
    }

    pub fn clear(&self) -> crate::Result<()> {
        match &self.clipboard {
            Ok(clipboard) => clipboard
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .clear()
                .map_err(Into::into),
            Err(e) => Err(crate::Error::Clipboard(e.to_string())),
        }
    }

    /// Warning: This method should not be used on the main thread! Otherwise the underlying libraries may deadlock on Linux, freezing the whole app, when trying to copy data copied from this app, for example if the user copies text from the WebView.
    pub fn read_image(&self) -> crate::Result<Image<'_>> {
        match &self.clipboard {
            Ok(clipboard) => {
                let image = clipboard.lock().unwrap().as_mut().unwrap().get_image()?;
                let image = Image::new_owned(
                    image.bytes.to_vec(),
                    image.width as u32,
                    image.height as u32,
                );
                Ok(image)
            }
            Err(e) => Err(crate::Error::Clipboard(e.to_string())),
        }
    }

    pub(crate) fn cleanup(&self) {
        if let Ok(clipboard) = &self.clipboard {
            clipboard.lock().unwrap().take();
        }
    }
}

// ─── OHOS: no arboard; write_image via TSFN bridge, other methods unsupported ───

#[cfg(target_env = "ohos")]
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Clipboard<R>> {
    // Register the Rust-side Clipboard bridge plugin so ArkTS configurePlugins
    // can match it. Without this, bridge calls fail with
    // "Bridge plugin 'ohos.clipboard' is not installed for '<module>'".
    use openharmony_ability_plugin_clipboard::ClipboardBridgePlugin;
    if let Ok(guard) = tauri::ohos::APP.lock() {
        if let Some(ohos_app) = guard.as_ref() {
            if let Err(e) = ohos_app.register_plugin(ClipboardBridgePlugin) {
                log::error!(
                    "[clipboard-manager] failed to register ClipboardBridgePlugin: {}",
                    e
                );
            }
        }
    }
    Ok(Clipboard { app: app.clone() })
}

#[cfg(target_env = "ohos")]
/// Access to the clipboard APIs.
///
/// On OHOS, `write_text`, `read_text`, and `write_image` are supported via
/// the bridge plugin facade (`openharmony-ability-plugin-clipboard`).
/// Other methods (`write_html`, `clear`, `read_image`) return `PlatformNotAvailable`.
pub struct Clipboard<R: Runtime> {
    #[allow(dead_code)]
    app: AppHandle<R>,
}

#[cfg(target_env = "ohos")]
impl<R: Runtime> Clipboard<R> {
    // write_text on OHOS: bridge plugin via openharmony-ability-plugin-clipboard.
    // Command handler runs on a worker thread, so block_on is safe.
    pub fn write_text<'a, T: Into<Cow<'a, str>>>(&self, text: T) -> crate::Result<()> {
        use openharmony_ability_plugin_clipboard::ClipboardExt;

        let client = tauri::ohos::APP
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|app| app.clone()))
            .and_then(|app| app.clipboard().ok())
            .ok_or_else(|| {
                crate::Error::Clipboard(
                    "Failed to create ClipboardClient: OHOS APP not initialized".to_string(),
                )
            })?;
        let text = text.into().to_string();
        futures_executor::block_on(client.write_text(text))
            .map_err(|e| crate::Error::Clipboard(e.to_string()))
    }

    // read_text on OHOS: bridge plugin via openharmony-ability-plugin-clipboard.
    pub fn read_text(&self) -> crate::Result<String> {
        use openharmony_ability_plugin_clipboard::ClipboardExt;

        let client = tauri::ohos::APP
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|app| app.clone()))
            .and_then(|app| app.clipboard().ok())
            .ok_or_else(|| {
                crate::Error::Clipboard(
                    "Failed to create ClipboardClient: OHOS APP not initialized".to_string(),
                )
            })?;
        futures_executor::block_on(client.read_text())
            .map(|opt| opt.unwrap_or_default())
            .map_err(|e| crate::Error::Clipboard(e.to_string()))
    }

    // write_image on OHOS: bridge plugin facade via openharmony-ability-plugin-clipboard.
    // RGBA data is extracted in commands.rs before the .await boundary
    // (the ResourceTable MutexGuard is !Send and cannot cross .await).
    pub async fn write_image(&self, rgba: &[u8], width: u32, height: u32) -> crate::Result<()> {
        use openharmony_ability_plugin_clipboard::ClipboardExt;

        let client = tauri::ohos::APP
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|app| app.clone()))
            .and_then(|app| app.clipboard().ok())
            .ok_or_else(|| {
                crate::Error::Clipboard(
                    "Failed to create ClipboardClient: OHOS APP not initialized".to_string(),
                )
            })?;
        client
            .write_image(rgba, width, height)
            .await
            .map_err(|e| crate::Error::Clipboard(e.to_string()))?;
        Ok(())
    }

    // write_html on OHOS: bridge plugin facade via openharmony-ability-plugin-clipboard.
    // Uses block_on (same pattern as read_text) — clipboard commands run on a
    // worker thread, not the main thread, so blocking on the bridge call is safe.
    pub fn write_html<'a, T: Into<Cow<'a, str>>>(
        &self,
        html: T,
        _alt_text: Option<T>,
    ) -> crate::Result<()> {
        use openharmony_ability_plugin_clipboard::ClipboardExt;

        let client = tauri::ohos::APP
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|app| app.clone()))
            .and_then(|app| app.clipboard().ok())
            .ok_or_else(|| {
                crate::Error::Clipboard(
                    "Failed to create ClipboardClient: OHOS APP not initialized".to_string(),
                )
            })?;
        futures_executor::block_on(client.write_html(html.into().to_string()))
            .map_err(|e| crate::Error::Clipboard(e.to_string()))?;
        Ok(())
    }

    // clear on OHOS: bridge plugin facade via openharmony-ability-plugin-clipboard.
    pub fn clear(&self) -> crate::Result<()> {
        use openharmony_ability_plugin_clipboard::ClipboardExt;

        let client = tauri::ohos::APP
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|app| app.clone()))
            .and_then(|app| app.clipboard().ok())
            .ok_or_else(|| {
                crate::Error::Clipboard(
                    "Failed to create ClipboardClient: OHOS APP not initialized".to_string(),
                )
            })?;
        futures_executor::block_on(client.clear())
            .map_err(|e| crate::Error::Clipboard(e.to_string()))?;
        Ok(())
    }

    /// Warning: This method should not be used on the main thread! Otherwise the underlying libraries may deadlock on Linux, freezing the whole app, when trying to copy data copied from this app, for example if the user copies text from the WebView.
    // TODO: Add TSFN bridge for read_image on OHOS
    pub fn read_image(&self) -> crate::Result<Image<'_>> {
        Err(crate::Error::Clipboard(
            "read_image not supported on OHOS (only write_image is available)".to_string(),
        ))
    }

    pub(crate) fn cleanup(&self) {
        // No arboard clipboard to clean up on OHOS
    }
}
