// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
pub mod webassembly;

#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
pub use webassembly::*;

#[cfg(not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"))))]
pub mod desktop;

#[cfg(not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"))))]
pub use desktop::*;
