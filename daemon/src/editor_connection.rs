use std::{collections::HashMap, env, path::PathBuf};

use anyhow::Context;
use tracing::debug;

use crate::{
    ot::OTServer,
    path::{AbsolutePath, FileUri, RelativePath},
    sandbox,
    types::{
        ComponentMessage, EditorProtocolMessageError, EditorProtocolMessageFromEditor,
        EditorProtocolMessageToEditor, RevisionedEditorTextDelta,
    },
};

/// Represents a connection to an editor. Handles the OT. To keep the code testable and sync, we do
/// the actual sending of messages in the daemon, and the functions here just *calculate* them.
pub struct EditorConnection {
    id: String,
    // TODO: Feels a bit duplicated here?
    base_dir: PathBuf,
    /// There's one OTServer per open buffer.
    ot_servers: HashMap<RelativePath, OTServer>,
}

impl EditorConnection {
    pub fn new(id: String, base_dir: PathBuf) -> Self {
        Self {
            id,
            base_dir,
            ot_servers: HashMap::new(),
        }
    }

    pub fn owns(&self, file_path: &RelativePath) -> bool {
        self.ot_servers.contains_key(file_path)
    }

    pub fn message_from_daemon(
        &mut self,
        message: &ComponentMessage,
    ) -> Vec<EditorProtocolMessageToEditor> {
        match message {
            ComponentMessage::Edit { file_path, delta } => {
                if let Some(ot_server) = self.ot_servers.get_mut(file_path) {
                    debug!("Applying incoming CRDT patch for {file_path:?}");
                    let rev_text_delta_for_editor = ot_server.apply_crdt_change(delta);

                    let uri = AbsolutePath::from_parts(&self.base_dir, file_path)
                        .expect("Should be able to construct absolute URI")
                        .to_file_uri();

                    vec![EditorProtocolMessageToEditor::Edit {
                        uri: uri.to_string(),
                        delta: rev_text_delta_for_editor.delta,
                        revision: rev_text_delta_for_editor.revision,
                    }]
                } else {
                    // We don't have the file open, just do nothing.
                    vec![]
                }
            }
            ComponentMessage::Cursor {
                file_path,
                ranges,
                name,
                cursor_id,
            } => {
                let uri = AbsolutePath::from_parts(&self.base_dir, file_path)
                    .expect("Should be able to construct absolute URI")
                    .to_file_uri();

                vec![EditorProtocolMessageToEditor::Cursor {
                    name: name.clone(),
                    userid: cursor_id.clone(),
                    uri: uri.to_string(),
                    ranges: ranges.clone(),
                }]
            }
            _ => {
                debug!("Ignoring message from inside: {message:#?}");
                vec![]
            }
        }
    }

    pub fn message_from_editor(
        &mut self,
        message: &EditorProtocolMessageFromEditor,
    ) -> Result<(ComponentMessage, Vec<EditorProtocolMessageToEditor>), EditorProtocolMessageError>
    {
        fn anyhow_err_to_protocol_err(error: anyhow::Error) -> EditorProtocolMessageError {
            EditorProtocolMessageError {
                code: -1, // TODO: Should the error codes differ per error?
                message: error.to_string(),
                data: None,
            }
        }

        match message {
            EditorProtocolMessageFromEditor::Open { uri } => {
                let uri = FileUri::try_from(uri.clone()).map_err(anyhow_err_to_protocol_err)?;
                let absolute_path = uri.to_absolute_path();
                let relative_path = RelativePath::try_from_absolute(&absolute_path, &self.base_dir)
                    .map_err(anyhow_err_to_protocol_err)?;

                debug!("Got an 'open' message for {relative_path:?}");
                if !sandbox::exists(&self.base_dir, &absolute_path)
                    .map_err(anyhow_err_to_protocol_err)?
                {
                    // Creating nonexisting files allows us to traverse this file for whether it's
                    // ignored, which is needed to even be allowed to open it.
                    sandbox::write_file(&self.base_dir, &absolute_path, b"")
                        .map_err(anyhow_err_to_protocol_err)?;
                }

                // We only want to process these messages for files that are not ignored.
                if sandbox::ignored(&self.base_dir, &absolute_path)
                    .expect("Could not check ignore status of opened file")
                {
                    return Err(EditorProtocolMessageError {
                        code: -1,
                        message: format!("File {absolute_path:?} is ignored"),
                        data: Some("This file should not be shared with other peers".into()),
                    });
                }

                let bytes = sandbox::read_file(&self.base_dir, &absolute_path)
                    .map_err(anyhow_err_to_protocol_err)?;
                let text = String::from_utf8(bytes)
                    .context("Failed to convert bytes to string")
                    .map_err(anyhow_err_to_protocol_err)?;

                let ot_server = OTServer::new(text);
                self.ot_servers.insert(relative_path.clone(), ot_server);

                Ok((
                    ComponentMessage::Open {
                        file_path: relative_path,
                    },
                    vec![],
                ))
            }
            EditorProtocolMessageFromEditor::Close { uri } => {
                let uri = FileUri::try_from(uri.clone()).map_err(anyhow_err_to_protocol_err)?;
                let absolute_path = uri.to_absolute_path();
                let relative_path = RelativePath::try_from_absolute(&absolute_path, &self.base_dir)
                    .map_err(anyhow_err_to_protocol_err)?;

                debug!("Got a 'close' message for {relative_path:?}");
                self.ot_servers.remove(&relative_path);

                Ok((
                    ComponentMessage::Close {
                        file_path: relative_path,
                    },
                    vec![],
                ))
            }
            EditorProtocolMessageFromEditor::Edit {
                uri,
                revision,
                delta,
            } => {
                debug!(
                    "Handling RevDelta from editor: revision {:#?}, delta {:#?}",
                    revision, delta
                );

                let uri = FileUri::try_from(uri.clone()).map_err(anyhow_err_to_protocol_err)?;
                let absolute_path = uri.to_absolute_path();
                let relative_path = RelativePath::try_from_absolute(&absolute_path, &self.base_dir)
                    .map_err(anyhow_err_to_protocol_err)?;

                if self.ot_servers.get_mut(&relative_path).is_none() {
                    return Err(EditorProtocolMessageError {
                        code: -1,
                        message: "File not found".into(),
                        data: Some(
                            "Please stop sending edits for this file or 'open' it before.".into(),
                        ),
                    });
                }

                let ot_server = self
                    .ot_servers
                    .get_mut(&relative_path)
                    .expect("Could not find OT server.");

                let rev_delta = RevisionedEditorTextDelta {
                    revision: *revision,
                    delta: delta.clone(),
                };

                let (delta_for_crdt, rev_deltas_for_editor) =
                    ot_server.apply_editor_operation(rev_delta.clone());

                let uri = AbsolutePath::from_parts(&self.base_dir, &relative_path)
                    .expect("Should be able to construct absolute URI")
                    .to_file_uri();

                let messages_to_editor = rev_deltas_for_editor
                    .into_iter()
                    .map(|rev_delta_for_editor| EditorProtocolMessageToEditor::Edit {
                        uri: uri.to_string(),
                        delta: rev_delta_for_editor.delta,
                        revision: rev_delta_for_editor.revision,
                    })
                    .collect();

                Ok((
                    ComponentMessage::Edit {
                        file_path: relative_path,
                        delta: delta_for_crdt,
                    },
                    messages_to_editor,
                ))
            }
            EditorProtocolMessageFromEditor::Cursor { uri, ranges } => {
                let uri = FileUri::try_from(uri.clone()).map_err(anyhow_err_to_protocol_err)?;
                let absolute_path = uri.to_absolute_path();
                let relative_path = RelativePath::try_from_absolute(&absolute_path, &self.base_dir)
                    .map_err(anyhow_err_to_protocol_err)?;

                Ok((
                    ComponentMessage::Cursor {
                        cursor_id: self.id.clone(),
                        name: env::var("USER").ok(),
                        file_path: relative_path,
                        ranges: ranges.clone(),
                    },
                    vec![],
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::factories::*;
    use pretty_assertions::assert_eq;
    use temp_dir::TempDir;

    #[test]
    fn opening_file_in_wrong_dir_fails() {
        let dir = TempDir::new().expect("Failed to create temp directory");
        let mut editor_connection =
            EditorConnection::new("1".to_string(), dir.path().to_path_buf());

        let result =
            editor_connection.message_from_editor(&EditorProtocolMessageFromEditor::Open {
                uri: "file:///foobar/file".to_string(),
            });

        assert!(result.is_err());
    }

    #[test]
    fn edits_are_oted() {
        let dir = TempDir::new().expect("Failed to create temp directory");
        let file = dir.path().join("file");
        std::fs::write(&file, "hello").expect("Failed to write file");

        let mut editor_connection =
            EditorConnection::new("1".to_string(), dir.path().to_path_buf());

        // Editor opens the file.
        let result =
            editor_connection.message_from_editor(&EditorProtocolMessageFromEditor::Open {
                uri: format!("file://{}", file.display()),
            });
        assert_eq!(
            result,
            Ok((
                ComponentMessage::Open {
                    file_path: RelativePath::new("file")
                },
                vec![]
            ))
        );

        // Daemon sends an edit.
        let delta = insert(1, "x"); // hello -> hxello
        let result = editor_connection.message_from_daemon(&ComponentMessage::Edit {
            file_path: RelativePath::new("file"),
            delta,
        });
        assert_eq!(
            result,
            vec![EditorProtocolMessageToEditor::Edit {
                uri: format!("file://{}", file.display()),
                revision: 0,
                delta: ed_delta_single((0, 1), (0, 1), "x")
            }]
        );

        // Editor sends an edit.
        let result =
            editor_connection.message_from_editor(&EditorProtocolMessageFromEditor::Edit {
                uri: format!("file://{}", file.display()),
                revision: 0,
                delta: ed_delta_single((0, 3), (0, 3), "y"),
            });
        let (inside_message, messages_to_editor) = result.unwrap();
        let delta = insert(4, "y"); // Position gets transformed!
        assert_eq!(
            inside_message,
            ComponentMessage::Edit {
                file_path: RelativePath::new("file"),
                delta
            }
        );
        assert_eq!(
            messages_to_editor,
            vec![EditorProtocolMessageToEditor::Edit {
                uri: format!("file://{}", file.display()),
                revision: 1,
                delta: ed_delta_single((0, 1), (0, 1), "x") // Delta is still the
                                                            // same.
            }]
        );
    }
}
