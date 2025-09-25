// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later
use ethersync_shared::keypair::Keypair;
use iroh::SecretKey;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use tracing::debug;

pub fn get_keypair_from_basedir(base_dir: &Path) -> Keypair {
    let keyfile = base_dir.join(".ethersync").join("key");
    if keyfile.exists() {
        let metadata =
            fs::metadata(&keyfile).expect("Expected to have access to metadata of the keyfile");

        let current_permissions = metadata.permissions().mode();
        let allowed_permissions = 0o100_600;
        assert!(current_permissions == allowed_permissions, "For security reasons, please make sure to set the key file to user-readable only (set the permissions to 600).");

        assert!(metadata.len() == 64, "Your keyfile is not 64 bytes long. This is a sign that it was created by an Ethersync version older than 0.7.0, which is not compatible. Please remove .ethersync/key, and try again.");

        debug!("Re-using existing keypair.");
        let mut file = File::open(keyfile).expect("Failed to open key file");

        let mut secret_key = [0; 32];
        file.read_exact(&mut secret_key)
            .expect("Failed to read from key file");

        let mut passphrase = [0; 32];
        file.read_exact(&mut passphrase)
            .expect("Failed to read from key file");

        Keypair {
            secret_key: SecretKey::from_bytes(&secret_key),
            passphrase: SecretKey::from_bytes(&passphrase),
        }
    } else {
        debug!("Generating new keypair.");
        let secret_key = SecretKey::generate(rand::rngs::OsRng);
        let passphrase = SecretKey::generate(rand::rngs::OsRng);

        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(keyfile)
            .expect("Should have been able to create key file that did not exist before");

        file.write_all(&secret_key.to_bytes())
            .expect("Failed to write to key file");
        file.write_all(&passphrase.to_bytes())
            .expect("Failed to write to key file");

        Keypair {
            secret_key,
            passphrase,
        }
    }
}
