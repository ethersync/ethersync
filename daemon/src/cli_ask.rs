// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{bail, Result};
use std::io::Write;

pub fn ask(question: &str) -> Result<bool> {
    print!("{question} (y/N): ");
    std::io::stdout().flush()?;
    let mut lines = std::io::stdin().lines();
    if let Some(Ok(line)) = lines.next() {
        match line.to_lowercase().as_str() {
            "y" | "yes" => Ok(true),
            _ => Ok(false),
        }
    } else {
        bail!("Failed to read answer");
    }
}
