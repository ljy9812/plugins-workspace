## Context

dialog 插件提供文件选择（open）、文件保存（save）和消息弹框（message）三个核心功能。原始代码通过 `#[cfg(desktop)]` / `#[cfg(mobile)]` 区分平台实现：

- **desktop**: 使用 `rfd` crate（原生文件对话框）+ 窗口 parent 绑定
- **mobile** (Android/iOS): 使用 `run_mobile_plugin` 调用 Kotlin/Swift 原生插件

OHOS 基于 Linux 内核，`target_os = "linux"`，因此 `cfg(desktop) = true`。但 OHOS 不能使用 `rfd`（不支持），也不能使用桌面窗口 parent，应走 mobile 路径（ArkTS 插件）。通过 `target_env = "ohos"` 区分。

当前 commit `d6f84f40` 完成了 Rust 侧全部适配。ArkTS 侧实现位于 tauri 仓库的 `crates/tauri-cli/templates/mobile/open-harmony/dialog/` 模板目录中（详见 Decision 5）。

## Goals / Non-Goals

**Goals:**
- 记录 commit `d6f84f40` 中 Rust 侧所有实际修改
- 标注已知问题和未完成的 ArkTS 部分

**Non-Goals:**
- 修改 tauri 核心 crate 的 OHOS 基础设施
- 在 plugins-workspace 中维护 ArkTS 代码（由 tauri 仓库模板管理）

## Decisions

### Decision 1: cfg 条件编译映射规则

OHOS 的 `cfg(desktop) = true` 但行为应等同于 mobile。映射规则：

```
#[cfg(desktop)]           → #[cfg(all(desktop, not(target_env = "ohos")))]
#[cfg(mobile)]            → #[cfg(any(mobile, target_env = "ohos"))]
#[cfg_attr(mobile, ...)]  → #[cfg_attr(any(mobile, target_env = "ohos"), ...)]
```

**理由**: OHOS 不能使用 rfd（无 X11/Wayland），不能使用 raw_window_handle parent，应走 `run_mobile_plugin` 路径调用 ArkTS 插件。

### Decision 2: invoke_handler 使用 `not(target_os = "android")` 而非 `all(desktop, not(target_env = "ohos"))`

实际代码中 `init()` 使用 `#[cfg(not(target_os = "android"))]` 控制 invoke_handler 注册：

```rust
#[cfg(not(target_os = "android"))]
{
    builder = builder.invoke_handler(tauri::generate_handler![
        commands::open,
        commands::save,
        commands::message,
    ]);
}
```

这意味着 OHOS 上仍注册 Rust invoke_handler，命令走 Rust → `run_mobile_plugin` → ArkTS 路径。Android 不注册是因为 Android 有独立的 JS 脚本处理。

### Decision 3: FileDialogBuilder::new 中 parent 初始化已修复

结构体字段已更新为 `#[cfg(all(desktop, not(target_env = "ohos")))]`，`new()` 中对应初始化也已同步修正。

**状态**: 已修复。

### Decision 4: OHOS 插件标识符格式

```rust
#[cfg(target_env = "ohos")]
const PLUGIN_IDENTIFIER: &str = "@tauri/plugin-dialog";
```

OHOS 使用 npm 风格标识符（`@tauri/plugin-xxx`），与 Android 的 `app.tauri.xxx` 不同。

## Risks / Trade-offs

- [Resolved] `openharmony/` ArkTS 目录在 plugins-workspace 中不存在 — **实际实现在 tauri 仓库**（见下方 Decision 5）
- [Fixed] `FileDialogBuilder::new` parent 初始化 cfg 不一致 → **已修复**，`new()` 中 `#[cfg(desktop)]` 改为 `#[cfg(all(desktop, not(target_env = "ohos")))]`
- [Info] `invoke_handler` 使用 `not(target_os = "android")` 条件，OHOS 上 Rust handler 有效，设计正确，命令走 Rust → `run_mobile_plugin` → ArkTS 路径

## Decision 5: ArkTS 插件实现在 tauri 仓库

dialog 的 ArkTS 插件代码**不在 plugins-workspace 仓库中**，而是在 tauri 核心仓库中，通过 `tauri-cli` 模板机制生成到应用的 OHOS 工程：

**模板位置** (tauri 仓库):
```
crates/tauri-cli/templates/mobile/open-harmony/dialog/
├── build-profile.json5
├── hvigorfile.ts
├── oh-package.json5
└── src/main/ets/
    ├── index.ets          # 模块入口，re-export DialogPlugin
    └── Plugin.ets         # 插件实现（DialogPlugin class）
```

**生成位置** (应用工程中):
```
src-tauri/gen/ohos/dialog/src/main/ets/
├── index.ets
└── Plugin.ets
```

**Plugin.ets 实现概要**:
- `DialogPlugin` 继承 `Plugin`（来自 `@tauri/app`）
- 注册 3 个 ArkTS 命令：`showFilePicker`、`saveFileDialog`、`showMessageDialog`
- 使用 OHOS 系统 API：`@ohos.file.picker`（文件选择/保存）、`@ohos.promptAction`（消息弹框）
- 通过 `invoke.resolve()` / `invoke.reject()` 返回结果给 Rust 层

**`build.rs` 中 `.ohos_path("openharmony")` 的作用**：
plugins-workspace 的 `build.rs` 注册的 `openharmony` 路径是插件构建时的元数据声明，指向应用工程中生成的 ArkTS 模块目录名。实际 ArkTS 代码由 tauri 仓库的 `tauri-cli` 在 `tauri init` 时从模板生成到 `gen/ohos/dialog/` 下，最终拷贝到 OHOS 工程的 `openharmony/` 目录。

## Open Questions

- ~~`openharmony/` ArkTS 实现应从 tauri 仓库 PR #7 同步还是重新实现？~~ **已解决**: ArkTS 实现在 tauri 仓库 `crates/tauri-cli/templates/mobile/open-harmony/dialog/` 中，通过模板机制生成。
