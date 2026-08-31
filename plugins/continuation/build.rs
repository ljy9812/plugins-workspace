// Copyright 2019-2024 Tauri Programme within the Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const COMMANDS: &[&str] = &[
  "is_continuation_restore",
  "get_continuation_data",
  "set_continuation_data",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
