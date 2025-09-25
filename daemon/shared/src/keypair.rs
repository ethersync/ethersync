// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later
use iroh::SecretKey;

pub struct Keypair {
    pub secret_key: SecretKey,
    pub passphrase: SecretKey
}
