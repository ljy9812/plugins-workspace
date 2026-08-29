// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const COMMANDS: &[&str] = &[
    "get_font_scale",
    "is_screen_reader_enabled",
    "is_touch_explore_enabled",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
