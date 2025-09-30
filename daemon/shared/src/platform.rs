// SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
pub(crate) use dioxus_core::prelude::spawn;

#[cfg(not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"))))]
pub(crate) use tokio::spawn;

#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
pub(crate) use dioxus_hooks::UnboundedSender as Sender;

#[cfg(not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"))))]
pub(crate) use tokio::sync::mpsc::Sender;
