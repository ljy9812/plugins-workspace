// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const COMMANDS: &[&str] = &["capture_webview", "pick_color"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
