## ADDED Requirements

### Requirement: Workspace tauri-plugin dependency uses version with patch
The workspace `Cargo.toml` SHALL use a version dependency for `tauri-plugin` with a `[patch.crates-io]` section pointing to the fork that provides OHOS-specific APIs (`register_ohos_plugin()`, `ohos_path()`).

#### Scenario: Workspace dependency configured
- **WHEN** building any plugin in the workspace
- **THEN** `tauri-plugin = "2.5"` is declared in `[workspace.dependencies]`, and `[patch.crates-io]` patches it to `https://github.com/Eulogizethesun/tauri` branch `ohdev`

### Requirement: dialog Cargo.toml declares openharmony platform support
`plugins/dialog/Cargo.toml` SHALL declare `openharmony` in `[package.metadata.platforms.support]` with partial support level.

#### Scenario: Platform metadata present
- **WHEN** querying platform support for dialog plugin
- **THEN** `openharmony = { level = "partial", notes = "Does not support folder picker" }` is present

### Requirement: OHOS target dependency on tauri with wry feature
`plugins/dialog/Cargo.toml` SHALL add `tauri` with `wry` feature for `cfg(target_env = "ohos")`.

#### Scenario: OHOS dependency resolves
- **WHEN** compiling with `target_env = "ohos"`
- **THEN** `tauri` with `wry` feature is included

### Requirement: rfd excluded from OHOS builds
The `rfd` crate dependency SHALL be gated with `not(target_env = "ohos")` to prevent inclusion on OHOS.

#### Scenario: rfd not compiled for OHOS
- **WHEN** compiling with `target_env = "ohos"`
- **THEN** `rfd` crate is not included in dependencies

### Requirement: build.rs registers ohos_path
`plugins/dialog/build.rs` SHALL call `.ohos_path("openharmony")` in the tauri_plugin Builder.

#### Scenario: Build configuration includes OHOS path
- **WHEN** build.rs executes
- **THEN** `.ohos_path("openharmony")` is called on the Builder

### Requirement: Permission schema includes openHarmony target
`plugins/dialog/permissions/schemas/schema.json` SHALL include `openHarmony` in the Target enum.

#### Scenario: Permission target validation
- **WHEN** validating permissions with target `openHarmony`
- **THEN** schema accepts `openHarmony` as valid target

### Requirement: OpenResponse Folders/Folder variants excluded from OHOS
`OpenResponse::Folders` and `OpenResponse::Folder` enum variants SHALL be gated with `#[cfg(all(desktop, not(target_env = "ohos")))]`.

#### Scenario: OHOS cannot return folder responses
- **WHEN** compiling for OHOS target
- **THEN** `Folders` and `Folder` variants are not available in `OpenResponse`

### Requirement: Dead code fields allowed on OHOS
Fields `recursive`, `picker_mode`, `file_access_mode`, and `SaveDialogOptions` struct SHALL use `#[cfg_attr(any(mobile, target_env = "ohos"), allow(dead_code))]`.

#### Scenario: No dead code warnings on OHOS
- **WHEN** compiling for OHOS target
- **THEN** no dead_code warnings are emitted for these fields

### Requirement: set_default_path has OHOS and desktop variants
Two `set_default_path` functions SHALL exist: one for `any(mobile, target_env = "ohos")` (simple file name only), one for `all(desktop, not(target_env = "ohos"))` (with directory + separator handling).

#### Scenario: OHOS uses mobile set_default_path
- **WHEN** compiling for OHOS target
- **THEN** the mobile variant of `set_default_path` is used

### Requirement: open command directory branch returns FolderPickerNotImplemented on OHOS
The `open` command SHALL return `Error::FolderPickerNotImplemented` when `directory=true` on OHOS.

#### Scenario: Folder picker not supported
- **WHEN** `open` is called with `directory=true` on OHOS
- **THEN** `Err(Error::FolderPickerNotImplemented)` is returned

### Requirement: save and message commands exclude parent window on OHOS
`save` and `message` commands SHALL NOT call `set_parent` / `parent` on OHOS.

#### Scenario: No parent window binding on OHOS
- **WHEN** `save` or `message` is called on OHOS
- **THEN** no `set_parent(&window)` or `builder.parent(&window)` call is made

### Requirement: Module declarations use OHOS-aware cfg
`mod desktop` SHALL use `#[cfg(all(desktop, not(target_env = "ohos")))]`, `mod mobile` SHALL use `#[cfg(any(mobile, target_env = "ohos"))]`.

#### Scenario: OHOS uses mobile module
- **WHEN** compiling for OHOS target
- **THEN** `mod mobile` is included, `mod desktop` is excluded

### Requirement: CANCEL/YES/NO constants available on OHOS
String constants `CANCEL`, `YES`, `NO` SHALL be gated with `#[cfg(any(mobile, target_env = "ohos"))]`.

#### Scenario: Constants available for OHOS payload
- **WHEN** compiling for OHOS target
- **THEN** `CANCEL`, `YES`, `NO` constants are available

### Requirement: MessageDialogBuilder parent field excluded on OHOS
`MessageDialogBuilder::parent` field and `parent()` method SHALL be gated with `#[cfg(all(desktop, not(target_env = "ohos")))]`.

#### Scenario: No parent field on OHOS
- **WHEN** compiling for OHOS target
- **THEN** `MessageDialogBuilder` has no `parent` field

### Requirement: FileDialogBuilder parent and folder methods excluded on OHOS
`FileDialogBuilder::parent` field, `set_parent()`, `pick_folder()`, `pick_folders()`, `blocking_pick_folder()`, `blocking_pick_folders()` SHALL be gated with `#[cfg(all(desktop, not(target_env = "ohos")))]`.

#### Scenario: No folder picker or parent on OHOS
- **WHEN** compiling for OHOS target
- **THEN** folder picker methods and parent field/method are not available

### Requirement: init() registers invoke_handler for non-Android
`init()` SHALL register invoke_handler with `#[cfg(not(target_os = "android"))]`, meaning OHOS includes Rust invoke_handler.

#### Scenario: OHOS has invoke_handler registered
- **WHEN** plugin initializes on OHOS
- **THEN** `commands::open`, `commands::save`, `commands::message` are registered

### Requirement: init() setup uses OHOS-aware module init
`setup()` SHALL call `mobile::init()` for `any(mobile, target_env = "ohos")` and `desktop::init()` for `all(desktop, not(target_env = "ohos"))`.

#### Scenario: OHOS uses mobile init
- **WHEN** plugin initializes on OHOS
- **THEN** `mobile::init(app, api)` is called

### Requirement: mobile.rs registers OHOS plugin with correct identifier
`mobile.rs` SHALL define `PLUGIN_IDENTIFIER = "@tauri/plugin-dialog"` for `target_env = "ohos"` and call `api.register_ohos_plugin(PLUGIN_IDENTIFIER, "DialogPlugin")`.

#### Scenario: OHOS plugin registration
- **WHEN** `mobile::init()` runs on OHOS
- **THEN** `register_ohos_plugin("@tauri/plugin-dialog", "DialogPlugin")` is called

### Requirement: ArkTS plugin implementation resides in tauri repo
The dialog ArkTS plugin code SHALL be maintained in the tauri core repository at `crates/tauri-cli/templates/mobile/open-harmony/dialog/src/main/ets/`, NOT in the plugins-workspace repository.

#### Scenario: ArkTS plugin files available via tauri-cli template
- **WHEN** `tauri init` generates an OHOS project for an app using the dialog plugin
- **THEN** `Plugin.ets` (DialogPlugin class) and `index.ets` are generated from the tauri repo template into `gen/ohos/dialog/src/main/ets/`

#### Scenario: ArkTS commands registered
- **WHEN** the DialogPlugin ArkTS class initializes on OHOS
- **THEN** three commands are registered: `showFilePicker` (uses `@ohos.file.picker`), `saveFileDialog` (uses `@ohos.file.picker`), `showMessageDialog` (uses `@ohos.promptAction`)
