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
    time::{Duration, SystemTime},
};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    time::sleep,
};
use tracing::debug;

#[derive(Debug, PartialEq, Eq)]
pub struct WatcherEvent {
    pub file_path: PathBuf,
    pub event_type: WatcherEventType,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum WatcherEventType {
    Created,
    Removed,
    Changed,
}

struct PendingEvent {
    event_type: WatcherEventType,
    timestamp: SystemTime,
}

#[derive(Clone)]
enum TimeoutEvent {
    PendingEvent {
        file_path: PathBuf,
        event_type: WatcherEventType,
    },
    None,
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
        let mut watcher = notify::recommended_watcher(move |res: NotifyResult<notify::Event>| {
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
            let (duration, event) = self.upcoming_timeout();
            tokio::select! {
                () = sleep(duration) => {
                    match event {
                        TimeoutEvent::PendingEvent { file_path, event_type } => {
                            // Removed triggered pending event.
                            self.pending_events.remove(&file_path);
                            self.event_tx.send(WatcherEvent{
                                file_path,
                                event_type,
                            }).await.expect("Channel closed");
                        },
                        TimeoutEvent::None => {
                            panic!("Watcher timed out without an event. This is a bug.");
                        }
                    }
                }
                event = self.notify_receiver.recv() => {
                    // TODO: Better errors?
                    let event = event.unwrap().unwrap();
                    match event.kind {
                        EventKind::Create(notify::event::CreateKind::File) => {
                            assert!(event.paths.len() == 1);
                            self.maybe_created(&event.paths[0]);
                        }
                        EventKind::Remove(notify::event::RemoveKind::File) => {
                            assert!(event.paths.len() == 1);
                            self.maybe_removed(&event.paths[0]);
                        }
                        EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                            assert!(event.paths.len() == 1);
                            self.maybe_modified(&event.paths[0]);
                        }
                        EventKind::Modify(notify::event::ModifyKind::Name(
                            notify::event::RenameMode::Both,
                        )) => {
                            assert!(event.paths.len() == 2);
                            self.maybe_removed(&event.paths[0]);
                            self.maybe_created(&event.paths[1]);
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
                                        self.maybe_created(&file_path);
                                    } else {
                                        self.maybe_removed(&file_path);
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

    fn upcoming_timeout(&self) -> (Duration, TimeoutEvent) {
        let next_pending_maybe = self
            .pending_events
            .iter()
            .min_by_key(|(_, pending_event)| pending_event.timestamp);
        if let Some((next_file_path, next_pending_event)) = next_pending_maybe {
            let pending_event = TimeoutEvent::PendingEvent {
                file_path: next_file_path.clone(),
                event_type: next_pending_event.event_type.clone(),
            };

            let now = SystemTime::now();
            match next_pending_event.timestamp.duration_since(now) {
                Ok(duration) => (duration, pending_event),
                // If duration_since fails, the timestamp is already in the past.
                // Trigger it immediately.
                Err(_) => (Duration::from_secs(0), pending_event),
            }
        } else {
            (Duration::MAX, TimeoutEvent::None)
        }
    }

    fn maybe_created(&mut self, file_path: &Path) {
        match sandbox::ignored(&self.app_config, file_path) {
            Ok(is_ignored) => {
                if is_ignored {
                    debug!("Ignoring creation of '{}'", file_path.display());
                    return;
                }
                // We only dispatch the event, if ignore check worked and it's not ignored.
            }
            Err(error) => {
                debug!(
                    "Ignoring creation of '{}' because of an error: {}",
                    file_path.display(),
                    error
                );
                return;
            }
        }

        self.add_pending(file_path, WatcherEventType::Created);
    }

    fn maybe_removed(&mut self, file_path: &Path) {
        // TODO: We should check whether the file was ignored here. But how?
        self.add_pending(file_path, WatcherEventType::Removed);
    }

    fn maybe_modified(&mut self, file_path: &Path) {
        match sandbox::ignored(&self.app_config, file_path) {
            Ok(is_ignored) => {
                if is_ignored {
                    debug!("Ignoring modification of '{}'", file_path.display());
                    return;
                }
                // We only dispatch the event, if ignore check worked and it's not ignored.
            }
            Err(error) => {
                debug!(
                    "Ignoring modification of '{}' because of an error: {}",
                    file_path.display(),
                    error
                );
                return;
            }
        }

        self.add_pending(file_path, WatcherEventType::Changed);
    }

    fn add_pending(&mut self, file_path: &Path, mut event_type: WatcherEventType) {
        if let Some(pending_event) = self.pending_events.get(file_path) {
            event_type = match (pending_event.event_type.clone(), event_type.clone()) {
                (WatcherEventType::Created, WatcherEventType::Changed) => {
                    // Keep the type at "Created", even if it is modified afterwards.
                    WatcherEventType::Created
                }
                (
                    WatcherEventType::Removed | WatcherEventType::Changed,
                    WatcherEventType::Created,
                ) => {
                    // Because the file seems to have existed before, change the type to "Changed".
                    WatcherEventType::Changed
                }
                _ => {
                    // Set the desired `event_type`.
                    event_type
                }
            }
        }

        let timestamp = SystemTime::now() + Duration::from_millis(100);
        self.pending_events.insert(
            file_path.into(),
            PendingEvent {
                event_type,
                timestamp,
            },
        );
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
            Some(WatcherEvent {
                file_path: file,
                event_type: WatcherEventType::Created,
            })
        );
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn change() {
        let (_dir, dir_path, app_config) = create_temp_dir_and_app_config();

        let mut file = dir_path.clone();
        file.push("file");
        sandbox::write_file(&dir_path, &file, b"hi").unwrap();

        let mut watcher = Watcher::spawn(app_config);

        sandbox::write_file(&dir_path, &file, b"yo").unwrap();

        assert_eq!(
            watcher.recv().await,
            Some(WatcherEvent {
                file_path: file,
                event_type: WatcherEventType::Changed,
            })
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
            Some(WatcherEvent {
                file_path: file,
                event_type: WatcherEventType::Removed,
            })
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
            Some(WatcherEvent {
                file_path: file,
                event_type: WatcherEventType::Removed,
            })
        );

        assert_eq!(
            watcher.recv().await,
            Some(WatcherEvent {
                file_path: file_new,
                event_type: WatcherEventType::Created,
            })
        );
    }

    #[tokio::test]
    async fn ignore() {
        let (_dir, dir_path, app_config) = create_temp_dir_and_app_config();

        let mut gitignore = dir_path.clone();
        gitignore.push(".ignore");
        sandbox::write_file(&dir_path, &gitignore, b"file").unwrap();

        sleep(Duration::from_millis(100)).await;
        let mut watcher = Watcher::spawn(app_config);

        let mut file = dir_path.clone();
        file.push("file");
        let mut file2 = dir_path.clone();
        file2.push("file2");

        sandbox::write_file(&dir_path, &file, b"hi").unwrap();
        sandbox::write_file(&dir_path, &file2, b"ho").unwrap();

        assert_eq!(
            watcher.recv().await,
            Some(WatcherEvent {
                file_path: file2,
                event_type: WatcherEventType::Created,
            })
        );
    }

    #[tokio::test]
    async fn remove_then_create() {
        let (_dir, dir_path, app_config) = create_temp_dir_and_app_config();

        let mut file = dir_path.clone();
        file.push("file");
        sandbox::write_file(&dir_path, &file, b"hi").unwrap();

        let mut watcher = Watcher::spawn(app_config);

        sandbox::remove_file(&dir_path, &file).unwrap();
        sandbox::write_file(&dir_path, &file, b"i'm back").unwrap();

        assert_eq!(
            watcher.recv().await,
            Some(WatcherEvent {
                file_path: file,
                event_type: WatcherEventType::Changed,
            })
        );
    }
}
