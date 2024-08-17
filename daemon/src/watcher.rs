#![allow(dead_code)]

use crate::sandbox;
use notify::{
    event::EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult,
    Watcher as NotifyWatcher,
};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::{self, Receiver};
use tracing::info;

#[derive(Debug, PartialEq)]
enum WatcherEvent {
    Created {
        file_path: String,
        content: String,
    },
    Removed {
        file_path: String,
    },
    Changed {
        file_path: String,
        new_content: String,
    },
}

struct Watcher {
    watcher: RecommendedWatcher,
    base_dir: PathBuf,
    rx: Receiver<NotifyResult<notify::Event>>,
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
            watcher,
            base_dir: dir.to_path_buf(),
            rx,
        }
    }
    async fn next(&mut self) -> Option<WatcherEvent> {
        let event = self.rx.recv().await.unwrap().unwrap();

        match event.kind {
            EventKind::Remove(notify::event::RemoveKind::File) => {
                assert!(event.paths.len() == 1);
                let file_path = event.paths[0]
                    .to_str()
                    .expect("Failed to convert path to string")
                    .into();

                Some(WatcherEvent::Removed { file_path })
            }
            EventKind::Create(notify::event::CreateKind::File) => {
                assert!(event.paths.len() == 1);
                let file_path = event.paths[0]
                    .to_str()
                    .expect("Failed to convert path to string")
                    .into();

                let bytes = sandbox::read_file(&self.base_dir, &event.paths[0])
                    .expect("Failed to read created file");
                let content = String::from_utf8(bytes).expect("Could not read file as UTF-8");

                Some(WatcherEvent::Created { file_path, content })
            }
            EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                assert!(event.paths.len() == 1);
                let file_path = event.paths[0]
                    .to_str()
                    .expect("Failed to convert path to string")
                    .into();

                let bytes = sandbox::read_file(&self.base_dir, &event.paths[0])
                    .expect("Failed to read created file");
                let content = String::from_utf8(bytes).expect("Could not read file as UTF-8");

                Some(WatcherEvent::Changed {
                    file_path,
                    new_content: content,
                })
            }
            /*
            TODO: We need to split those into two events...

            EventKind::Modify(notify::event::ModifyKind::Name(
                notify::event::RenameMode::Both,
            )) => {
                assert!(event.paths.len() == 2);
                let from_path = &event.paths[0];
                let to_path = &event.paths[1];

                // TODO: Avoid repeating the effects here.
                // TODO: Is there a cleverer way to do renames in the CRDT?

                document_handle
                    .send_message(DocMessage::RemoveFile {
                        file_path: from_path
                            .to_str()
                            .expect("Failed to convert path to string")
                            .into(),
                    })
                    .await;

                if !sandbox::ignored(&base_dir_cloned, to_path)
                    .expect("Could not determine ignored status of file")
                {
                    let bytes = sandbox::read_file(&base_dir_cloned, to_path)
                        .expect("Failed to read created file");
                    let content = String::from_utf8(bytes).expect("Could not read file as UTF-8");

                    document_handle
                        .send_message(DocMessage::CreateFile {
                            file_path: to_path
                                .to_str()
                                .expect("Failed to convert path to string")
                                .into(),
                            content,
                        })
                        .await;
                }
            }*/
            e => {
                // Don't handle other events.
                // But log them! I'm curious what they are!
                info!("{:?}: {e:?}", event.paths);
                None
            }
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
                file_path: file.as_path().to_str().unwrap().to_string(), // oof
                content: "hi".to_string()
            })
        );
    }
}
