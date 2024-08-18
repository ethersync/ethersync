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
                e => {
                    // Don't handle other events.
                    // But log them! I'm curious what they are!
                    debug!("Unhandled event in {:?}: {e:?}", event.paths);
                    continue;
                }
            };
        }
    }

    fn maybe_created(&self, file_path: &Path) -> Option<WatcherEvent> {
        if sandbox::ignored(&self.base_dir, file_path)
            .expect("Could not check ignore state of file")
        {
            return None;
        }

        let content =
            sandbox::read_file(&self.base_dir, file_path).expect("Failed to read created file");

        Some(WatcherEvent::Created {
            file_path: file_path.to_path_buf(),
            content,
        })
    }

    fn maybe_removed(&self, file_path: &Path) -> Option<WatcherEvent> {
        // TODO: We should check whether the file was ignored here. But how?
        Some(WatcherEvent::Removed {
            file_path: file_path.to_path_buf(),
        })
    }

    fn maybe_modified(&self, file_path: &Path) -> Option<WatcherEvent> {
        if sandbox::ignored(&self.base_dir, file_path)
            .expect("Could not check ignore state of file")
        {
            return None;
        }

        let content =
            sandbox::read_file(&self.base_dir, file_path).expect("Failed to read created file");

        Some(WatcherEvent::Changed {
            file_path: file_path.to_path_buf(),
            new_content: content,
        })
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

    #[tokio::test]
    async fn ignore() {
        let dir = TempDir::new().expect("Failed to create temp directory");
        let gitignore = dir.child(".ignore");

        sandbox::write_file(dir.path(), &gitignore, b"file").unwrap();

        let mut watcher = Watcher::new(dir.path());

        let file = dir.child("file");
        let file2 = dir.child("file2");

        sandbox::write_file(dir.path(), &file, b"hi").unwrap();
        sandbox::write_file(dir.path(), &file2, b"ho").unwrap();

        assert_eq!(
            watcher.next().await,
            Some(WatcherEvent::Created {
                file_path: file2,
                content: b"ho".to_vec()
            })
        );
    }
}
