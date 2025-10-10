// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{config::AppConfig, sandbox};
use notify::{
    event::EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult,
    Watcher as NotifyWatcher,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::debug;

// TODO: refactor: use WatcherEventType.
#[derive(Debug, PartialEq, Eq)]
pub enum WatcherEvent {
    Created { file_path: PathBuf },
    Removed { file_path: PathBuf },
    Changed { file_path: PathBuf },
}

#[derive(Debug, PartialEq, Eq)]
enum WatcherEventType {
    Created,
    Removed,
    Changed,
}

struct PendingEvent {
    event: WatcherEventType,
    timestamp: SystemTime,
}

/// Returns events among the files in `base_dir` that are not ignored.
#[must_use]
pub struct Watcher {
    _inner: RecommendedWatcher,
    app_config: AppConfig,
    notify_receiver: Receiver<NotifyResult<notify::Event>>,
    event_tx: Sender<WatcherEvent>,
    pending_events: HashMap<PathBuf, PendingEvent>,
}

impl Watcher {
    pub fn spawn(app_config: AppConfig) -> Receiver<WatcherEvent> {
        let (event_tx, event_rx) = mpsc::channel(1);

        let (tx, rx) = mpsc::channel(1);
        let mut watcher =
            notify::recommended_watcher(move |res: NotifyResult<notify::Event>| {
                futures::executor::block_on(async {
                    tx.send(res).await.unwrap();
                });
            })
            .expect("Could not construct watcher");

        watcher
            .watch(&app_config.base_dir, RecursiveMode::Recursive)
            .expect("Failed to watch directory");

        let mut watcher = Self {
            // Keep the watcher, so that it's not dropped.
            _inner: watcher,
            app_config,
            notify_receiver: rx,
            event_tx,
            pending_events: HashMap::default(),
        };

        tokio::spawn(async move {
            watcher.run().await;
        });

        event_rx
    }

    async fn run(&mut self) {
        loop {
            tokio::select! {
                event = self.notify_receiver.recv() => {
                    // TODO: Better errors?
                    let event = event.unwrap().unwrap();
                    match event.kind {
                        EventKind::Create(notify::event::CreateKind::File) => {
                            assert!(event.paths.len() == 1);
                            if let Some(e) = self.maybe_created(&event.paths[0]) {
                                self.event_tx.send(e).await.expect("Channel closed");
                            }
                        }
                        EventKind::Remove(notify::event::RemoveKind::File) => {
                            assert!(event.paths.len() == 1);
                            if let Some(e) = self.maybe_removed(&event.paths[0]) {
                                self.event_tx.send(e).await.expect("Channel closed");
                            }
                        }
                        EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                            assert!(event.paths.len() == 1);
                            if let Some(e) = self.maybe_modified(&event.paths[0]) {
                                self.event_tx.send(e).await.expect("Channel closed");
                            }
                        }
                        EventKind::Modify(notify::event::ModifyKind::Name(
                            notify::event::RenameMode::Both,
                        )) => {
                            assert!(event.paths.len() == 2);
                            let removed = self.maybe_removed(&event.paths[0]);
                            let created = self.maybe_created(&event.paths[1]);

                            if let Some(e) = removed {
                                self.event_tx.send(e).await.expect("Channel closed");
                            }
                            if let Some(e) = created {
                                self.event_tx.send(e).await.expect("Channel closed");
                            }
                        }
                        // MacOS doesn't give us details on moving a file, so we need to infer what
                        // happened.
                        EventKind::Modify(notify::event::ModifyKind::Name(
                            notify::event::RenameMode::Any,
                        )) => {
                            assert!(event.paths.len() == 1);
                            let file_path = event.paths[0].clone();
                            match sandbox::exists(&self.app_config.base_dir, &file_path) {
                                Ok(path_exists) => {
                                    if path_exists {
                                        if let Some(e) = self.maybe_created(&file_path) {
                                            self.event_tx.send(e).await.expect("Channel closed");
                                        }
                                    } else if let Some(e) = self.maybe_removed(&file_path) {
                                        self.event_tx.send(e).await.expect("Channel closed");
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
                        }
                    }
                }
            };
        }
    }

    #[must_use]
    fn maybe_created(&self, file_path: &Path) -> Option<WatcherEvent> {
        match sandbox::ignored(&self.app_config, file_path) {
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

    #[must_use]
    #[expect(clippy::unnecessary_wraps, clippy::unused_self)]
    fn maybe_removed(&self, file_path: &Path) -> Option<WatcherEvent> {
        // TODO: We should check whether the file was ignored here. But how?
        Some(WatcherEvent::Removed {
            file_path: file_path.to_path_buf(),
        })
    }

    #[must_use]
    fn maybe_modified(&self, file_path: &Path) -> Option<WatcherEvent> {
        match sandbox::ignored(&self.app_config, file_path) {
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
mod tests {
    use temp_dir::TempDir;

    use super::*;

    fn create_temp_dir_and_app_config() -> (TempDir, PathBuf, AppConfig) {
        let dir = TempDir::new().expect("Failed to create temp directory");

        // We canonicalize the path here, because on macOS, TempDir gives us paths in /var/, which
        // symlinks to /private/var/. But the paths in the file events are always in /private/var/.
        // If we wouldn't canonicalize, the watcher would ignore basically all events.
        let dir_path = dir.path().canonicalize().unwrap();

        let app_config = AppConfig {
            base_dir: dir_path.clone(),
            ..Default::default()
        };

        (dir, dir_path, app_config)
    }

    #[tokio::test]
    async fn create() {
        let (_dir, dir_path, app_config) = create_temp_dir_and_app_config();

        let mut file = dir_path.clone();
        file.push("file");

        let mut watcher = Watcher::spawn(app_config);
        sandbox::write_file(&dir_path, &file, b"hi").unwrap();

        assert_eq!(
            watcher.recv().await,
            Some(WatcherEvent::Created {
                // TODO: Should the file_paths maybe be relative to the base dir already?
                file_path: file,
            })
        );
    }

    #[tokio::test]
    async fn change() {
        let (_dir, dir_path, app_config) = create_temp_dir_and_app_config();

        let mut file = dir_path.clone();
        file.push("file");
        sandbox::write_file(&dir_path, &file, b"hi").unwrap();

        let mut watcher = Watcher::spawn(app_config);

        sandbox::write_file(&dir_path, &file, b"yo").unwrap();

        assert_eq!(
            watcher.recv().await,
            Some(WatcherEvent::Changed { file_path: file })
        );
    }

    #[tokio::test]
    async fn remove() {
        let (_dir, dir_path, app_config) = create_temp_dir_and_app_config();

        let mut file = dir_path.clone();
        file.push("file");
        sandbox::write_file(&dir_path, &file, b"hi").unwrap();

        let mut watcher = Watcher::spawn(app_config);

        sandbox::remove_file(&dir_path, &file).unwrap();

        assert_eq!(
            watcher.recv().await,
            Some(WatcherEvent::Removed { file_path: file })
        );
    }

    #[tokio::test]
    async fn rename() {
        let (_dir, dir_path, app_config) = create_temp_dir_and_app_config();

        let mut file = dir_path.clone();
        file.push("file");
        let mut file_new = dir_path.clone();
        file_new.push("file2");
        sandbox::write_file(&dir_path, &file, b"hi").unwrap();

        let mut watcher = Watcher::spawn(app_config);

        sandbox::rename_file(&dir_path, &file, &file_new).unwrap();

        assert_eq!(
            watcher.recv().await,
            Some(WatcherEvent::Removed { file_path: file })
        );

        assert_eq!(
            watcher.recv().await,
            Some(WatcherEvent::Created {
                file_path: file_new,
            })
        );
    }

    #[tokio::test]
    async fn ignore() {
        let (_dir, dir_path, app_config) = create_temp_dir_and_app_config();

        let mut gitignore = dir_path.clone();
        gitignore.push(".ignore");
        sandbox::write_file(&dir_path, &gitignore, b"file").unwrap();

        let mut watcher = Watcher::spawn(app_config);

        let mut file = dir_path.clone();
        file.push("file");
        let mut file2 = dir_path.clone();
        file2.push("file2");

        sandbox::write_file(&dir_path, &file, b"hi").unwrap();
        sandbox::write_file(&dir_path, &file2, b"ho").unwrap();

        assert_eq!(
            watcher.recv().await,
            Some(WatcherEvent::Created { file_path: file2 })
        );
    }
}
