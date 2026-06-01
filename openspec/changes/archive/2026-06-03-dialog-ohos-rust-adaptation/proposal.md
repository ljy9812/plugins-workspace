## Why

Dialog 插件需要在 OHOS (OpenHarmony) 平台上运行。OHOS 基于 Linux 内核，`target_os` 为 "linux"，通过 `target_env = "ohos"` 区分。OHOS 被判定为 mobile 平台（`desktop=false`），因此所有 `#[cfg(desktop)]` 和 `#[cfg(mobile)]` 条件编译都需要调整以包含 OHOS。

当前 commit `d6f84f40` 是 PR #2 被 revert（PR #5）后的重新实现，完成了 Rust 侧的全部 cfg 条件编译适配和 mobile.rs 的 OHOS 插件注册，但 ArkTS 侧（`openharmony/` 目录）尚未创建。

## What Changes

- workspace `Cargo.toml`: `tauri-plugin` 使用 version 依赖 + `[patch.crates-io]` 指向 tauri fork（支持 `register_ohos_plugin()` 和 `ohos_path()` API）
- `plugins/dialog/Cargo.toml`: 添加 openharmony 平台声明、OHOS target 依赖、排除 rfd 在 OHOS 上使用
- `plugins/dialog/build.rs`: 添加 `.ohos_path("openharmony")` 构建配置
- `plugins/dialog/permissions/schemas/schema.json`: Target 枚举添加 `openHarmony`
- `plugins/dialog/src/commands.rs`: 11 处 cfg 条件编译变更 + 调试日志
- `plugins/dialog/src/lib.rs`: 20+ 处 cfg 条件编译变更 + init() 函数重构
- `plugins/dialog/src/mobile.rs`: OHOS 插件标识符 `@tauri/plugin-dialog` + `register_ohos_plugin()`
- `plugins/dialog/openharmony/`: **不在 plugins-workspace 中维护**，ArkTS 实现位于 tauri 仓库 `crates/tauri-cli/templates/mobile/open-harmony/dialog/`

## Capabilities

### New Capabilities
- `dialog-ohos-cfg-gates`: dialog 插件 Rust 侧 cfg 条件编译适配，使 desktop/mobile 分支正确处理 OHOS 平台

### Modified Capabilities

## Impact

- `Cargo.toml`: workspace 依赖变更
- `plugins/dialog/Cargo.toml`: 平台声明 + target 依赖
- `plugins/dialog/build.rs`: 构建配置
- `plugins/dialog/permissions/schemas/schema.json`: 权限 schema
- `plugins/dialog/src/commands.rs`: 命令实现
- `plugins/dialog/src/lib.rs`: 插件入口 + 公共 API
- `plugins/dialog/src/mobile.rs`: 移动端 / OHOS 初始化
