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
use tracing::info;

#[derive(Debug, PartialEq)]
enum WatcherEvent {
    Created {
        file_path: PathBuf,
        content: Vec<u8>,
    },
    Removed {
        file_path: PathBuf,
    },
    Changed {
        file_path: PathBuf,
        new_content: Vec<u8>,
    },
}

struct Watcher {
    watcher: RecommendedWatcher,
    base_dir: PathBuf,
    notify_receiver: Receiver<NotifyResult<notify::Event>>,
    out_queue: VecDeque<WatcherEvent>,
}

impl Watcher {
    fn new(dir: &Path) -> Self {
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
            watcher,
            base_dir: dir.to_path_buf(),
            notify_receiver: rx,
            out_queue: VecDeque::new(),
        }
    }

    async fn next(&mut self) -> Option<WatcherEvent> {
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
                    let file_path = event.paths[0].clone();

                    let content = sandbox::read_file(&self.base_dir, &event.paths[0])
                        .expect("Failed to read created file");

                    return Some(WatcherEvent::Created { file_path, content });
                }
                EventKind::Remove(notify::event::RemoveKind::File) => {
                    assert!(event.paths.len() == 1);
                    let file_path = event.paths[0].clone();

                    return Some(WatcherEvent::Removed { file_path });
                }
                EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                    assert!(event.paths.len() == 1);
                    let file_path = event.paths[0].clone();

                    let content = sandbox::read_file(&self.base_dir, &event.paths[0])
                        .expect("Failed to read created file");

                    return Some(WatcherEvent::Changed {
                        file_path,
                        new_content: content,
                    });
                }
                EventKind::Modify(notify::event::ModifyKind::Name(
                    notify::event::RenameMode::Both,
                )) => {
                    assert!(event.paths.len() == 2);
                    let from_path = event.paths[0].clone();
                    let to_path = event.paths[1].clone();

                    let remove_event = WatcherEvent::Removed {
                        file_path: from_path,
                    };

                    let content = sandbox::read_file(&self.base_dir, &event.paths[1])
                        .expect("Failed to read created file");
                    let create_event = WatcherEvent::Created {
                        file_path: to_path,
                        content,
                    };

                    // Queue the create event for later, return the remove event.
                    self.out_queue.push_back(create_event);
                    return Some(remove_event);
                }
                e => {
                    // Don't handle other events.
                    // But log them! I'm curious what they are!
                    info!("{:?}: {e:?}", event.paths);
                    continue;
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use temp_dir::TempDir;

    use super::*;

    #[tokio::test]
    async fn create() {
        let dir = TempDir::new().expect("Failed to create temp directory");
        let mut watcher = Watcher::new(dir.path());

        let file = dir.child("file");
        sandbox::write_file(dir.path(), &file, b"hi").unwrap();

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Created {
                // TODO: Should the file_paths maybe be relative to the base dir already?
                file_path: file,
                content: b"hi".to_vec()
            })
        );
    }

    #[tokio::test]
    async fn change() {
        let dir = TempDir::new().expect("Failed to create temp directory");

        let file = dir.child("file");
        sandbox::write_file(dir.path(), &file, b"hi").unwrap();

        let mut watcher = Watcher::new(dir.path());

        sandbox::write_file(dir.path(), &file, b"yo").unwrap();

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Changed {
                file_path: file,
                new_content: b"yo".to_vec()
            })
        );
    }

    #[tokio::test]
    async fn remove() {
        let dir = TempDir::new().expect("Failed to create temp directory");

        let file = dir.child("file");
        sandbox::write_file(dir.path(), &file, b"hi").unwrap();

        let mut watcher = Watcher::new(dir.path());

        sandbox::remove_file(dir.path(), &file).unwrap();

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Removed { file_path: file })
        );
    }

    #[tokio::test]
    async fn rename() {
        let dir = TempDir::new().expect("Failed to create temp directory");

        let file = dir.child("file");
        let file_new = dir.child("file2");
        sandbox::write_file(dir.path(), &file, b"hi").unwrap();

        let mut watcher = Watcher::new(dir.path());

        sandbox::rename_file(dir.path(), &file, &file_new).unwrap();

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Removed { file_path: file })
        );

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Created {
                file_path: file_new,
                content: b"hi".to_vec(),
            })
        );
    }
}
