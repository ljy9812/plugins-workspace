![plugin-stronghold](https://github.com/tauri-apps/plugins-workspace/raw/v2/plugins/stronghold/banner.png)

Store secrets and keys using the [IOTA Stronghold](https://github.com/iotaledger/stronghold.rs) secret management engine.

| Platform | Supported |
| -------- | --------- |
| Linux    | ✓         |
| Windows  | ✓         |
| macOS    | ✓         |
| Android  | ✓         |
| iOS      | ✓         |

## Install

_This plugin requires a Rust version of at least **1.77.2**_

There are three general methods of installation that we can recommend.

1. Use crates.io and npm (easiest, and requires you to trust that our publishing pipeline worked)
2. Pull sources directly from Github using git tags / revision hashes (most secure)
3. Git submodule install this repo in your tauri project and then use file protocol to ingest the source (most secure, but inconvenient to use)

Install the Core plugin by adding the following to your `Cargo.toml` file:

`src-tauri/Cargo.toml`

```toml
[dependencies]
tauri-plugin-stronghold = "2.0.0"
# alternatively with Git:
tauri-plugin-stronghold = { git = "https://github.com/tauri-apps/plugins-workspace", branch = "v2" }
```

Due to an [upstream bug](https://github.com/tauri-apps/plugins-workspace/issues/2048) we also recommend that you add this to your `Cargo.toml` file:

```toml
[profile.dev.package.scrypt]
opt-level = 3
```

You can install the JavaScript Guest bindings using your preferred JavaScript package manager:

> Note: If your JavaScript package manager cannot install packages from git monorepos, you can still use the code by manually copying the [Guest bindings](./guest-js/index.ts) into your source files.

```sh
pnpm add @tauri-apps/plugin-stronghold
# or
npm add @tauri-apps/plugin-stronghold
# or
yarn add @tauri-apps/plugin-stronghold
```

## Usage

First you need to register the core plugin with Tauri:

`src-tauri/src/lib.rs`

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_stronghold::Builder::new(|password| {
            // Hash the password here with e.g. argon2, blake2b or any other secure algorithm
            // Here is an example implementation using the `rust-argon2` crate for hashing the password

            use argon2::{hash_raw, Config, Variant, Version};

            let config = Config {
                lanes: 4,
                mem_cost: 10_000,
                time_cost: 10,
                variant: Variant::Argon2id,
                version: Version::Version13,
                ..Default::default()
            };

            let salt = "your-salt".as_bytes();

            let key = hash_raw(password.as_ref(), salt, &config).expect("failed to hash password");

            key.to_vec()
        })
        .build())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Afterwards all the plugin's APIs are available through the JavaScript guest bindings:

```javascript
import { Stronghold, Location, Client } from "tauri-plugin-stronghold-api";
import { appDataDir } from "@tauri-apps/api/path";

const initStronghold = async () => {
  const vaultPath = `${await appDataDir()}/vault.hold`;

  const vaultKey = "The key to the vault";

  const stronghold = await Stronghold.load(vaultPath, vaultKey);

  let client: Client;

  const clientName = "name your client";

  try {
    client = await hold.loadClient(clientName);
  } catch {
    client = await hold.createClient(clientName);
  }

  return {
    stronghold,
    client,
  };
};

const { stronghold, client } = await initStronghold();

const store = client.getStore();

const key = "my_key";

// Insert a record to the store

const data = Array.from(new TextEncoder().encode("Hello, World!"));

await store.insert(key, data);

// Read a record from store

const data = await store.get(key);

const value = new TextDecoder().decode(new Uint8Array(data));

// Save your updates

await stronghold.save();

// Remove a record from store

await store.remove(key);
```

## OHOS Build

Stronghold depends on `libsodium-sys-stable`, whose build script runs `./configure`
to compile libsodium from source. On Windows cross-compiling to OHOS,
`./configure` cannot run (os error 193), so a prebuilt libsodium is required.
On an OHOS PC the automatic source build works without any setup.

### Prerequisites

- A prebuilt `libsodium` for `aarch64-unknown-linux-ohos`. Sources:
  - OHOS PC Conan registry (`OpenHarmonyPCDeveloper/Conan`) or `cmd-pkgs` releases
  - Built once on an OHOS PC, or with the OHOS NDK clang: `./configure --host=aarch64-linux-ohos && make`
- When cross-compiling **from Windows**, additionally copy (or symlink) the
  static library to the Unix linker name in the same directory:

  ```bash
  cp libsodium.a liblibsodium.a
  ```

  `libsodium-sys-stable`'s build script picks the link name with
  `cfg!(target_env = "msvc")`, which reflects the *host* (Windows → `libsodium`),
  while the Unix-flavored OHOS *target* resolves `-l libsodium` to
  `liblibsodium.a`. Both names must exist. This is not needed on an OHOS PC,
  where the host already picks the `sodium` name.

### Steps

1. Point `SODIUM_LIB_DIR` at the directory containing `libsodium.a`
   (and its `liblibsodium.a` copy when cross-compiling from Windows):

   ```bash
   export SODIUM_LIB_DIR=/path/to/libsodium/lib
   ```

   `libsodium-sys-stable` picks the env var up directly and skips `./configure`.
   The plugin's `build.rs` fails fast with a hint when it is missing.

2. Build the plugin:

   ```bash
   cargo check -p tauri-plugin-stronghold --target aarch64-unknown-linux-ohos
   ```

The `stronghold-runtime` crate is consumed from the OpenHarmony Artifactory
cargo registry: the workspace `.cargo/config.toml` replaces crates.io with
the artifactory, which serves the OHOS adaptation — excluding the `nix`
dependency for OHOS and fixing a `DirectAlloc` misaligned-offset panic —
under the same version `2.0.1` with an adapted checksum (locally published
builds win over the registry's crates.io remote fallback), so no patch
entry is needed. If you consume this plugin in your own project, configure
the same source replacement (or otherwise point your build at the
artifactory) — without it the unadapted crates.io build of
`stronghold-runtime` pulls `nix` into the OHOS dependency graph. Remove the
replacement once upstream stronghold.rs ships an OHOS-compatible release.
The reference source of the adaptation is the `ohos` branch of the public
fork at <https://gitcode.com/dragonswordy/stronghold.rs>.

### Runtime notes

- The first snapshot `save()` and every snapshot load run scrypt at upstream's
  recommended work factor 19 (≈2^19 iterations). Measured on an aarch64
  HarmonyOS PC (debug build) this costs ≈107 s per call; release builds are
  expected to be in the ~1 s range per upstream's design. The cost is
  password hardening by design, not an OHOS regression. The full plugin chain
  (key provider, BIP39/SLIP-10/Ed25519
  procedures, vault + store, snapshot round-trip, wrong-password rejection) is
  verified end-to-end on an aarch64 HarmonyOS PC by
  `tests/ohos_e2e.rs`.

## Contributing

PRs accepted. Please make sure to read the Contributing Guide before making a pull request.

## Partners

<table>
  <tbody>
    <tr>
      <td align="center" valign="middle">
        <a href="https://crabnebula.dev" target="_blank">
          <img src="https://github.com/tauri-apps/plugins-workspace/raw/v2/.github/sponsors/crabnebula.svg" alt="CrabNebula" width="283">
        </a>
      </td>
    </tr>
  </tbody>
</table>

For the complete list of sponsors please visit our [website](https://tauri.app#sponsors) and [Open Collective](https://opencollective.com/tauri).

## License

Code: (c) 2015 - Present - The Tauri Programme within The Commons Conservancy.

MIT or MIT/Apache 2.0 where applicable.
