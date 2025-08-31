// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::sandbox;
use notify::{
    event::EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult,
    Watcher as NotifyWatcher,
};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};
use tokio::sync::mpsc::{self, Receiver};
use tracing::debug;

#[derive(Debug, PartialEq)]
pub enum WatcherEvent {
    Created { file_path: PathBuf },
    Removed { file_path: PathBuf },
    Changed { file_path: PathBuf },
}

/// Returns events among the files in base_dir that are not ignored.
pub struct Watcher {
    _watcher: RecommendedWatcher,
    base_dir: PathBuf,
    notify_receiver: Receiver<NotifyResult<notify::Event>>,
    out_queue: VecDeque<WatcherEvent>,
}

impl Watcher {
    pub fn new(dir: &Path) -> Self {
        let (tx, rx) = mpsc::channel(1);
        let mut watcher = notify::recommended_watcher(move |res: NotifyResult<notify::Event>| {
            futures::executor::block_on(async {
                tx.send(res).await.unwrap();
            });
        })
        .expect("Could not construct watcher");

        watcher
            .watch(dir, RecursiveMode::Recursive)
            .expect("Failed to watch directory");

        Self {
            // Keep the watcher, so that it's not dropped.
            _watcher: watcher,
            base_dir: dir.to_path_buf(),
            notify_receiver: rx,
            out_queue: VecDeque::new(),
        }
    }

    pub async fn next(&mut self) -> Option<WatcherEvent> {
        loop {
            // If there's an event in the queue, return the oldest one.
            if let Some(event) = self.out_queue.pop_front() {
                return Some(event);
            }

            // Otherwise, wait for the next event from the watcher.
            let event = self.notify_receiver.recv().await.unwrap().unwrap();

            match event.kind {
                EventKind::Create(notify::event::CreateKind::File) => {
                    assert!(event.paths.len() == 1);
                    if let Some(e) = self.maybe_created(&event.paths[0]) {
                        return Some(e);
                    }
                }
                EventKind::Remove(notify::event::RemoveKind::File) => {
                    assert!(event.paths.len() == 1);
                    if let Some(e) = self.maybe_removed(&event.paths[0]) {
                        return Some(e);
                    }
                }
                EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                    assert!(event.paths.len() == 1);
                    if let Some(e) = self.maybe_modified(&event.paths[0]) {
                        return Some(e);
                    }
                }
                EventKind::Modify(notify::event::ModifyKind::Name(
                    notify::event::RenameMode::Both,
                )) => {
                    assert!(event.paths.len() == 2);
                    let removed = self.maybe_removed(&event.paths[0]);
                    let created = self.maybe_created(&event.paths[1]);

                    // Queue the create event for later, return the remove event.
                    if let Some(e) = created {
                        self.out_queue.push_back(e);
                    }

                    if let Some(e) = removed {
                        return Some(e);
                    }
                }
                // MacOS doesn't give us details on moving a file, so we need to infer what
                // happened.
                EventKind::Modify(notify::event::ModifyKind::Name(
                    notify::event::RenameMode::Any,
                )) => {
                    assert!(event.paths.len() == 1);
                    let file_path = event.paths[0].clone();
                    match sandbox::exists(&self.base_dir, &file_path) {
                        Ok(path_exists) => {
                            if path_exists {
                                if let Some(e) = self.maybe_created(&file_path) {
                                    return Some(e);
                                }
                            } else if let Some(e) = self.maybe_removed(&file_path) {
                                return Some(e);
                            }
                        }
                        Err(error) => {
                            debug!(
                                "Ignoring creation/removal of '{}' because of an error: {}",
                                &file_path.display(),
                                error
                            );
                        }
                    }
                }
                EventKind::Access(_) => {
                    // We're not interested in these, ignore them.
                }
                e => {
                    // Don't handle other events.
                    // But log them! I'm curious what they are!
                    debug!("Unhandled event in {:?}: {e:?}", event.paths);
                    continue;
                }
            }
        }
    }

    fn maybe_created(&self, file_path: &Path) -> Option<WatcherEvent> {
        match sandbox::ignored(&self.base_dir, file_path) {
            Ok(is_ignored) => {
                if is_ignored {
                    debug!("Ignoring creation of '{}'", file_path.display());
                    return None;
                }
                // We only dispatch the event, if ignore check worked and it's not ignored.
            }
            Err(error) => {
                debug!(
                    "Ignoring creation of '{}' because of an error: {}",
                    file_path.display(),
                    error
                );
                return None;
            }
        }

        Some(WatcherEvent::Created {
            file_path: file_path.to_path_buf(),
        })
    }

    fn maybe_removed(&self, file_path: &Path) -> Option<WatcherEvent> {
        // TODO: We should check whether the file was ignored here. But how?
        Some(WatcherEvent::Removed {
            file_path: file_path.to_path_buf(),
        })
    }

    fn maybe_modified(&self, file_path: &Path) -> Option<WatcherEvent> {
        match sandbox::ignored(&self.base_dir, file_path) {
            Ok(is_ignored) => {
                if is_ignored {
                    debug!("Ignoring modification of '{}'", file_path.display());
                    return None;
                }
                // We only dispatch the event, if ignore check worked and it's not ignored.
            }
            Err(error) => {
                debug!(
                    "Ignoring modification of '{}' because of an error: {}",
                    file_path.display(),
                    error
                );
                return None;
            }
        }

        Some(WatcherEvent::Changed {
            file_path: file_path.to_path_buf(),
        })
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")] // TODO: For some reason, these tests hang forever on macOS.
mod tests {
    use temp_dir::TempDir;

    use super::*;

    #[tokio::test]
    async fn create() {
        let dir = TempDir::new().expect("Failed to create temp directory");

        // We canonicalize the path here, because on macOS, TempDir gives us paths in /var/, which
        // symlinks to /private/var/. But the paths in the file events are always in /private/var/.
        // If we wouldn't canonicalize, the watcher would ignore basically all events.
        let dir_path = dir.path().canonicalize().unwrap();
        let mut file = dir_path.clone();
        file.push("file");

        let mut watcher = Watcher::new(&dir_path);
        sandbox::write_file(&dir_path, &file, b"hi").unwrap();

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Created {
                // TODO: Should the file_paths maybe be relative to the base dir already?
                file_path: file,
            })
        );
    }

    #[tokio::test]
    async fn change() {
        let dir = TempDir::new().expect("Failed to create temp directory");

        let dir_path = dir.path().canonicalize().unwrap();
        let mut file = dir_path.clone();
        file.push("file");
        sandbox::write_file(&dir_path, &file, b"hi").unwrap();

        let mut watcher = Watcher::new(&dir_path);

        sandbox::write_file(&dir_path, &file, b"yo").unwrap();

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Changed { file_path: file })
        );
    }

    #[tokio::test]
    async fn remove() {
        let dir = TempDir::new().expect("Failed to create temp directory");

        let dir_path = dir.path().canonicalize().unwrap();
        let mut file = dir_path.clone();
        file.push("file");
        sandbox::write_file(&dir_path, &file, b"hi").unwrap();

        let mut watcher = Watcher::new(&dir_path);

        sandbox::remove_file(&dir_path, &file).unwrap();

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Removed { file_path: file })
        );
    }

    #[tokio::test]
    async fn rename() {
        let dir = TempDir::new().expect("Failed to create temp directory");

        let dir_path = dir.path().canonicalize().unwrap();
        let mut file = dir_path.clone();
        file.push("file");
        let mut file_new = dir_path.clone();
        file_new.push("file2");
        sandbox::write_file(&dir_path, &file, b"hi").unwrap();

        let mut watcher = Watcher::new(&dir_path);

        sandbox::rename_file(&dir_path, &file, &file_new).unwrap();

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Removed { file_path: file })
        );

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Created {
                file_path: file_new,
            })
        );
    }

    #[tokio::test]
    async fn ignore() {
        let dir = TempDir::new().expect("Failed to create temp directory");
        let dir_path = dir.path().canonicalize().unwrap();

        let mut gitignore = dir_path.clone();
        gitignore.push(".ignore");
        sandbox::write_file(&dir_path, &gitignore, b"file").unwrap();

        let mut watcher = Watcher::new(&dir_path);

        let mut file = dir_path.clone();
        file.push("file");
        let mut file2 = dir_path.clone();
        file2.push("file2");

        sandbox::write_file(&dir_path, &file, b"hi").unwrap();
        sandbox::write_file(&dir_path, &file2, b"ho").unwrap();

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Created { file_path: file2 })
        );
    }
}
