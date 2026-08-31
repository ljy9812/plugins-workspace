// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const COMMANDS: &[&str] = &["login", "silent_login", "logout"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
