// Copyright 2025 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! On-device end-to-end test for the OHOS revival.
//!
//! Drives the exact `iota_stronghold` API surface the plugin commands use —
//! `Stronghold::new` (fresh + snapshot reload), `create_client`/`load_client`,
//! the BIP39Generate → SLIP10Derive → Ed25519Sign procedure chain,
//! `vault().write_secret()`, `store().insert()/get()`, and `save()`
//! (`commit_with_keyprovider`) — compiled against the OHOS-adapted
//! stronghold-runtime supplied by the workspace source replacement
//! (`.cargo/config.toml` artifactory registry; see that file for why a
//! `[patch.crates-io]` entry is NOT used).
//!
//! The password is run through `kdf::KeyDerivation::argon2` with a persisted
//! salt file, mirroring the plugin's `initialize` command: `Stronghold::new`
//! takes the derived 32-byte key directly (`KeyProvider::try_from` rejects
//! any other length with `NCSizeNotAllowed` — the raw password never reaches
//! it in the real command flow).
//!
//! The positive read-back assertion is the signature: vault secrets are not
//! directly readable from the public API (`read_secret` is `#[cfg(test)]`
//! upstream), so the same message is signed with the reloaded key — an
//! identical signature proves the derived key material survived the snapshot
//! round-trip.
//!
//! Cross-compile and run on a HarmonyOS device:
//! ```text
//! cargo test -p tauri-plugin-stronghold --test ohos_e2e --no-run \
//!   --target aarch64-unknown-linux-ohos
//! # push the binary, then on device: chmod +x <bin> && <bin>
//! ```
//!
//! Runtime profile (aarch64 phone, debug build, measured 2026-09-08): all
//! crypto steps are sub-second except the snapshot `save()`/load round-trips —
//! upstream encrypts snapshots with scrypt at work factor 19 (≈2^19
//! iterations, by-design ~1s on a desktop release build), which costs ~107s
//! per call in an unoptimized build on a phone. The wrong-password and reload
//! phases each pay it again, so the whole test runs ≈5.5 minutes on device.
//! Passes with `--test-threads=1` and `HOME=/data/local/tmp` set (hdc shell
//! starts as root without a usable `$HOME`).

use std::path::PathBuf;

use crypto::keys::bip39;
use iota_stronghold::{
    procedures::{
        BIP39Generate, Curve, Ed25519Sign, MnemonicLanguage, ProcedureOutput, Slip10Derive,
        Slip10DeriveInput, StrongholdProcedure,
    },
    Location,
};
use zeroize::Zeroizing;

use tauri_plugin_stronghold::kdf::KeyDerivation;
use tauri_plugin_stronghold::stronghold::Stronghold;

fn snapshot_dir() -> PathBuf {
    // hdc shell runs as root; /data/local/tmp is the standard writable scratch
    // dir on device. Desktop hosts use the temp dir.
    if cfg!(target_env = "ohos") {
        PathBuf::from("/data/local/tmp")
    } else {
        std::env::temp_dir()
    }
}

fn sign(client: &iota_stronghold::Client, key: &Location) -> Vec<u8> {
    client
        .execute_procedure(StrongholdProcedure::Ed25519Sign(Ed25519Sign {
            private_key: key.clone(),
            msg: b"ohos-e2e-message".to_vec(),
        }))
        .unwrap()
        .into()
}

#[test]
fn snapshot_lifecycle_end_to_end() {
    let path = snapshot_dir().join(format!("stronghold-e2e-{}.snapshot", std::process::id()));
    let salt_path = snapshot_dir().join(format!("stronghold-e2e-{}.salt", std::process::id()));
    std::fs::remove_file(&path).ok();
    std::fs::remove_file(&salt_path).ok();

    // Mirror the plugin's `initialize` command: argon2 over the password with a
    // persisted salt yields the 32-byte key `Stronghold::new` requires. The
    // salt file survives across phases so the right password maps to the same
    // key, and a wrong one to a different (valid-length) key.
    let password = "ohos-e2e-password";
    let derive_key = |pw: &str| KeyDerivation::argon2(pw, &salt_path);
    let mut t = std::time::Instant::now();
    assert_eq!(derive_key(password).len(), 32);
    eprintln!("[timing] argon2_kdf #1 {:?}", t.elapsed());

    let client_id: Vec<u8> = b"ohos-e2e-client".to_vec();
    let vault_id: Vec<u8> = b"ohos-e2e-vault".to_vec();
    let record_id: Vec<u8> = b"ohos-e2e-record".to_vec();

    let seed_loc = Location::generic(vault_id.clone(), b"e2e-bip39-seed".to_vec());
    let key_loc = Location::generic(vault_id.clone(), b"e2e-slip10-key".to_vec());

    // Phase 1: fresh stronghold — run the procedure chain, write a raw secret
    // and a store entry, then persist the snapshot.
    let mut t = std::time::Instant::now();
    macro_rules! step {
        ($name:expr) => {{
            eprintln!("[timing] {} {:>9.3?}", $name, t.elapsed());
            t = std::time::Instant::now();
        }};
    }
    let signature_before = {
        let stronghold = Stronghold::new(&path, derive_key(password)).unwrap();
        step!("Stronghold::new");
        let client = stronghold.create_client(client_id.clone()).unwrap();
        step!("create_client");

        client
            .execute_procedure(StrongholdProcedure::BIP39Generate(BIP39Generate {
                passphrase: bip39::Passphrase::from(String::new()),
                output: seed_loc.clone(),
                language: MnemonicLanguage::English,
            }))
            .unwrap();
        step!("BIP39Generate");

        client
            .execute_procedure(StrongholdProcedure::Slip10Derive(Slip10Derive {
                curve: Curve::Ed25519,
                chain: vec![],
                input: Slip10DeriveInput::Seed(seed_loc.clone()),
                output: key_loc.clone(),
            }))
            .unwrap();
        step!("Slip10Derive");

        let signature = sign(&client, &key_loc);
        assert_eq!(signature.len(), 64, "ed25519 signature must be 64 bytes");
        step!("Ed25519Sign");

        client
            .vault(vault_id.clone())
            .write_secret(
                Location::generic(vault_id.clone(), record_id.clone()),
                Zeroizing::new(b"top-secret-payload".to_vec()),
            )
            .unwrap();
        step!("write_secret");
        let old = client
            .store()
            .insert(b"store-key".to_vec(), b"store-value".to_vec(), None)
            .unwrap();
        assert!(old.is_none());
        step!("store_insert");

        stronghold.save().unwrap();
        step!("save");
        signature
    };

    assert!(path.exists(), "snapshot file must exist after save");

    // Phase 2: a wrong password must fail to load — both keys are valid
    // argon2 derivatives of the same salt, so the failure comes from the
    // snapshot decryption itself, proving the file is actually encrypted
    // with the key provider.
    assert!(
        Stronghold::new(&path, derive_key("wrong-password")).is_err(),
        "loading with a wrong password must fail"
    );
    step!("wrong_password_load");

    // Phase 3: reopen with the right password — the client state loads back
    // from the snapshot, the store entry reads back, and signing with the
    // reloaded key yields the same signature.
    {
        let stronghold = Stronghold::new(&path, derive_key(password)).unwrap();
        step!("reload");
        let client = stronghold.load_client(client_id).unwrap();
        step!("load_client");

        let value = client.store().get(b"store-key").unwrap();
        assert_eq!(value.as_deref(), Some(&b"store-value"[..]));
        step!("store_get");

        let signature_after = sign(&client, &key_loc);
        step!("sign_after");
        assert_eq!(
            signature_before, signature_after,
            "signature with the reloaded key must match"
        );

        // The directly-written secret record also survived the round-trip:
        // revoking it and collecting the garbage must succeed.
        client
            .vault(vault_id.clone())
            .revoke_secret(record_id.clone())
            .unwrap();
        client.vault(vault_id).cleanup().unwrap();
        step!("revoke_cleanup");
    }

    std::fs::remove_file(&path).ok();
    std::fs::remove_file(&salt_path).ok();
}
