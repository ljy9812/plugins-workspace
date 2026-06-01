## 1. Workspace 依赖配置

- [x] 1.1 修改 workspace `Cargo.toml` 中 `tauri-plugin` 依赖为 version `"2.5"` + `[patch.crates-io]` 指向 fork
  - **文件**: `Cargo.toml`
  - **实际代码**: `tauri-plugin = "2.5"` + `[patch.crates-io] tauri-plugin = { git = "https://github.com/Eulogizethesun/tauri", branch = "ohdev" }`

## 2. dialog Cargo.toml 平台配置

- [x] 2.1 在 `[package.metadata.platforms.support]` 添加 `openharmony = { level = "partial", notes = "Does not support folder picker" }`
  - **文件**: `plugins/dialog/Cargo.toml`
- [x] 2.2 添加 `[target.'cfg(target_env = "ohos")'.dependencies]` 节，依赖 `tauri = { workspace = true, features = ["wry"] }`
  - **文件**: `plugins/dialog/Cargo.toml`
- [x] 2.3 修改 rfd 依赖 cfg 条件，添加 `not(target_env = "ohos")` 排除 OHOS
  - **文件**: `plugins/dialog/Cargo.toml`
  - **实际代码**: `[target."cfg(all(any(target_os = \"macos\", windows, target_os = \"linux\", ...), not(target_env = \"ohos\")))".dependencies]`

## 3. build.rs 构建配置

- [x] 3.1 在 Builder 链中添加 `.ohos_path("openharmony")`
  - **文件**: `plugins/dialog/build.rs`
  - **位置**: `.ios_path("ios")` 之后

## 4. 权限 Schema

- [x] 4.1 在 `schema.json` 的 Target 枚举 `anyOf` 数组中添加 `openHarmony` 项
  - **文件**: `plugins/dialog/permissions/schemas/schema.json`
  - **内容**: `{ "description": "OpenHarmony.", "type": "string", "enum": ["openHarmony"] }`

## 5. commands.rs cfg 条件编译

- [x] 5.1 `OpenResponse::Folders` 和 `OpenResponse::Folder`: `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`
  - **文件**: `plugins/dialog/src/commands.rs`
- [x] 5.2 `recursive`/`picker_mode`/`file_access_mode` 字段: `#[cfg_attr(mobile, ...)]` → `#[cfg_attr(any(mobile, target_env = "ohos"), ...)]`
  - **文件**: `plugins/dialog/src/commands.rs`
- [x] 5.3 `SaveDialogOptions` 结构体: `#[cfg_attr(mobile, ...)]` → `#[cfg_attr(any(mobile, target_env = "ohos"), ...)]`
  - **文件**: `plugins/dialog/src/commands.rs`
- [x] 5.4 `set_default_path` 两个版本: mobile 版 `#[cfg(mobile)]` → `#[cfg(any(mobile, target_env = "ohos"))]`，desktop 版 `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`
  - **文件**: `plugins/dialog/src/commands.rs`
- [x] 5.5 `open` 函数 directory 分支: desktop 版 `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`，mobile 版 `#[cfg(mobile)]` → `#[cfg(any(mobile, target_env = "ohos"))]`
  - **文件**: `plugins/dialog/src/commands.rs`
- [x] 5.6 `save` 函数 `set_parent`: `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`
  - **文件**: `plugins/dialog/src/commands.rs`
- [x] 5.7 `message` 函数 `parent`: `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`
  - **文件**: `plugins/dialog/src/commands.rs`
- [x] 5.8 `open` 函数添加调试日志 `log::info!("[dialog::open] command called, directory={}, multiple={}", ...)`
  - **文件**: `plugins/dialog/src/commands.rs`

## 6. lib.rs cfg 条件编译与 init 重构

- [x] 6.1 模块声明: `mod desktop` → `#[cfg(all(desktop, not(target_env = "ohos")))]`，`mod mobile` → `#[cfg(any(mobile, target_env = "ohos"))]`
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.2 use 声明和 pub use 声明同步更新 cfg
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.3 `CANCEL`/`YES`/`NO` 常量: `#[cfg(mobile)]` → `#[cfg(any(mobile, target_env = "ohos"))]`
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.4 `MessageDialogBuilder::parent` 字段: `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.5 `MessageDialogBuilder::new` 中 parent 初始化: `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.6 `MessageDialogPayload` 结构体和 `payload()` 方法: `#[cfg(mobile)]` → `#[cfg(any(mobile, target_env = "ohos"))]`
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.7 `MessageDialogBuilder::parent()` 方法: `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.8 `FileDialogBuilder::parent` 字段: `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.9 `FileDialogPayload` 结构体和 `payload()` 方法: `#[cfg(mobile)]` → `#[cfg(any(mobile, target_env = "ohos"))]`
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.10 `FileDialogBuilder::set_parent()`: `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.11 `pick_folder`/`pick_folders`/`blocking_pick_folder`/`blocking_pick_folders`: `#[cfg(desktop)]` → `#[cfg(all(desktop, not(target_env = "ohos")))]`
  - **文件**: `plugins/dialog/src/lib.rs`
- [x] 6.12 `init()` 中 `js_init_script` 和 `invoke_handler` 使用 `#[cfg(not(target_os = "android"))]` 包裹（OHOS 仍注册）
  - **文件**: `plugins/dialog/src/lib.rs`
  - **注意**: 条件为 `not(target_os = "android")`，非 `all(desktop, not(target_env = "ohos"))`
- [x] 6.13 `setup()` 中 `mobile::init` / `desktop::init` 使用 OHOS-aware cfg
  - **文件**: `plugins/dialog/src/lib.rs`

## 7. mobile.rs OHOS 插件注册

- [x] 7.1 添加 OHOS 插件标识符 `#[cfg(target_env = "ohos")] const PLUGIN_IDENTIFIER: &str = "@tauri/plugin-dialog";`
  - **文件**: `plugins/dialog/src/mobile.rs`
- [x] 7.2 添加 OHOS 插件注册 `#[cfg(target_env = "ohos")] let handle = api.register_ohos_plugin(PLUGIN_IDENTIFIER, "DialogPlugin")?;`
  - **文件**: `plugins/dialog/src/mobile.rs`
- [x] 7.3 更新注释 "initializes the Kotlin or Swift plugin classes" → "initializes the Kotlin, Swift or ArkTS plugin classes"
  - **文件**: `plugins/dialog/src/mobile.rs`

## 8. 已知问题（已修复）

- [x] 8.1 `FileDialogBuilder::new()` 中 `parent: None` 初始化从 `#[cfg(desktop)]` 修正为 `#[cfg(all(desktop, not(target_env = "ohos")))]`，与结构体字段声明保持一致
  - **文件**: `plugins/dialog/src/lib.rs`
  - **状态**: 已修复
