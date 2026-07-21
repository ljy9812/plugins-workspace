// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const COMMANDS: &[&str] = &[
    "initialize",
    "destroy",
    "save",
    "create_client",
    "load_client",
    "get_store_record",
    "save_store_record",
    "remove_store_record",
    "save_secret",
    "remove_secret",
    "execute_procedure",
];

fn main() {
    // OHOS: require prebuilt libsodium via SODIUM_LIB_DIR.
    // CARGO_CFG_TARGET_ENV reflects the TARGET triple (cross-compilation safe).
    // Do NOT use cfg!(target_env = "ohos") — build.rs runs on HOST.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("ohos") {
        println!("cargo:rerun-if-env-changed=SODIUM_LIB_DIR");
        match std::env::var("SODIUM_LIB_DIR") {
            Ok(dir) => {
                println!("cargo:warning=[stronghold] OHOS: using prebuilt libsodium from {dir}");
            }
            Err(_) => {
                panic!(
                    "OHOS target requires SODIUM_LIB_DIR to be set. \
                     libsodium-sys-stable's build.rs runs in a separate process and cannot \
                     read env vars set here; without SODIUM_LIB_DIR it will invoke ./configure \
                     and fail with os error 193. \
                     Run `bash scripts/build-libsodium-ohos.sh <target>` first, \
                     then `source scripts/env.sh`. \
                     See README.md OHOS section."
                );
            }
        }
    }

    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./api-iife.js")
        .build();
}
