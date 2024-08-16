use crate::{
    daemon::{DocMessage, DocumentActorHandle},
    sandbox,
};
use notify::{RecursiveMode, Result as NotifyResult, Watcher};
use std::{path::PathBuf, time::Duration};
use tracing::info;

pub async fn spawn_file_watcher(base_dir: PathBuf, document_handle: DocumentActorHandle) {
    let base_dir_cloned = base_dir.clone();
    let mut watcher = notify::recommended_watcher(move |res: NotifyResult<notify::Event>| {
        futures::executor::block_on(async {
            match res {
                Ok(event) => {
                    // TODO: On Linux, even a directory deletion seems to yield a Remove(File)?

                    match event.kind {
                        notify::event::EventKind::Remove(notify::event::RemoveKind::File) => {
                            for path in event.paths {
                                document_handle
                                    .send_message(DocMessage::RemoveFile {
                                        file_path: path
                                            .to_str()
                                            .expect("Failed to convert path to string")
                                            .into(),
                                    })
                                    .await;
                            }
                        }
                        notify::event::EventKind::Create(notify::event::CreateKind::File) => {
                            for path in event.paths {
                                if !sandbox::ignored(&base_dir_cloned, &path)
                                    .expect("Could not determine ignored status of file")
                                {
                                    let bytes = sandbox::read_file(&base_dir_cloned, &path)
                                        .expect("Failed to read created file");
                                    let content = String::from_utf8(bytes)
                                        .expect("Could not read file as UTF-8");

                                    document_handle
                                        .send_message(DocMessage::CreateFile {
                                            file_path: path
                                                .to_str()
                                                .expect("Failed to convert path to string")
                                                .into(),
                                            content,
                                        })
                                        .await;
                                }
                            }
                        }
                        notify::event::EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                            for path in event.paths {
                                if !sandbox::ignored(&base_dir_cloned, &path)
                                    .expect("Could not determine ignored status of file")
                                {
                                    let bytes = sandbox::read_file(&base_dir_cloned, &path)
                                        .expect("Failed to read created file");
                                    let content = String::from_utf8(bytes)
                                        .expect("Could not read file as UTF-8");

                                    document_handle
                                        .send_message(DocMessage::UpdateFile {
                                            file_path: path
                                                .to_str()
                                                .expect("Failed to convert path to string")
                                                .into(),
                                            content,
                                        })
                                        .await;
                                }
                            }
                        }
                        notify::event::EventKind::Modify(notify::event::ModifyKind::Name(
                            notify::event::RenameMode::Both,
                        )) => {
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
                                let content =
                                    String::from_utf8(bytes).expect("Could not read file as UTF-8");

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
                        }
                        e => {
                            // Don't handle other events.
                            // But log them! I'm curious what they are!
                            info!("{:?}: {e:?}", event.paths);
                        }
                    }
                }
                Err(e) => panic!("watch error: {e:?}"),
            }
        });
    })
    .expect("Failed to initialize file watcher");

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher
        .watch(&base_dir, RecursiveMode::Recursive)
        .expect("Failed to watch directory");

    // TODO: can this be event based?
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
