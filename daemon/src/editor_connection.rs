use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context};
use tracing::debug;

use crate::{
    ot::OTServer,
    sandbox,
    types::{
        EditorProtocolMessageError, EditorProtocolMessageFromEditor, EditorProtocolMessageToEditor,
        InsideMessage,
    },
};

/// Represents a connection to an editor. Handles the OT. To keep the code testable and sync, we do
/// the actual sending of messages in the daemon, and the functions here just *calculate* them.
pub struct EditorConnection {
    id: String,
    // TODO: Feels a bit duplicated here?
    base_dir: PathBuf,
    /// There's one OTServer per open buffer.
    ot_servers: HashMap<String, OTServer>,
}

impl EditorConnection {
    pub fn new(id: String, base_dir: &Path) -> Self {
        Self {
            id,
            base_dir: base_dir.to_owned(),
            ot_servers: HashMap::new(),
        }
    }

    pub fn owns(&self, file_path: &str) -> bool {
        self.ot_servers.contains_key(file_path)
    }

    pub fn message_from_daemon(
        &mut self,
        message: &InsideMessage,
    ) -> Vec<EditorProtocolMessageToEditor> {
        match message {
            InsideMessage::Edit { file_path, delta } => {
                if let Some(ot_server) = self.ot_servers.get_mut(file_path) {
                    debug!("Applying incoming CRDT patch for {file_path}");
                    let rev_text_delta_for_editor = ot_server.apply_crdt_change(delta);

                    vec![EditorProtocolMessageToEditor::Edit {
                        uri: format!("file://{}", self.absolute_path_for_file_path(file_path)),
                        delta: rev_text_delta_for_editor,
                    }]
                } else {
                    // We don't have the file open, just do nothing.
                    vec![]
                }
            }
            InsideMessage::Cursor {
                file_path,
                ranges,
                name,
                cursor_id,
            } => {
                let uri = format!("file://{}", self.absolute_path_for_file_path(file_path));
                vec![EditorProtocolMessageToEditor::Cursor {
                    name: name.clone(),
                    userid: cursor_id.clone(),
                    uri,
                    ranges: ranges.clone(),
                }]
            }
            _ => {
                debug!("Ignoring message from inside: {:#?}", message);
                vec![]
            }
        }
    }

    pub fn message_from_editor(
        &mut self,
        message: &EditorProtocolMessageFromEditor,
    ) -> Result<(InsideMessage, Vec<EditorProtocolMessageToEditor>), EditorProtocolMessageError>
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
                let file_path = self
                    .file_path_for_uri(uri)
                    .map_err(anyhow_err_to_protocol_err)?;

                debug!("Got an 'open' message for {file_path}");
                let absolute_file_path = self.absolute_path_for_file_path(&file_path);
                let absolute_file_path = Path::new(&absolute_file_path);
                if !sandbox::exists(&self.base_dir, absolute_file_path)
                    .map_err(anyhow_err_to_protocol_err)?
                {
                    // Creating nonexisting files allows us to traverse this file for whether it's
                    // ignored, which is needed to even be allowed to open it.
                    sandbox::write_file(&self.base_dir, absolute_file_path, b"")
                        .map_err(anyhow_err_to_protocol_err)?;
                }

                // We only want to process these messages for files that are not ignored.
                if sandbox::ignored(&self.base_dir, absolute_file_path)
                    .expect("Could not check ignore status of opened file")
                {
                    return Err(EditorProtocolMessageError {
                        code: -1,
                        message: format!("File '{absolute_file_path:?}' is ignored"),
                        data: Some("This file should not be shared with other peers".into()),
                    });
                }

                let bytes = sandbox::read_file(&self.base_dir, absolute_file_path)
                    .map_err(anyhow_err_to_protocol_err)?;
                let text = String::from_utf8(bytes)
                    .context("Failed to convert bytes to string")
                    .map_err(anyhow_err_to_protocol_err)?;

                let ot_server = OTServer::new(text);
                self.ot_servers.insert(file_path.clone(), ot_server);

                Ok((InsideMessage::Open { file_path }, vec![]))
            }
            EditorProtocolMessageFromEditor::Close { uri } => {
                let file_path = self
                    .file_path_for_uri(uri)
                    .map_err(anyhow_err_to_protocol_err)?;
                debug!("Got a 'close' message for {file_path}");
                self.ot_servers.remove(&file_path);

                Ok((InsideMessage::Close { file_path }, vec![]))
            }
            EditorProtocolMessageFromEditor::Edit {
                delta: rev_delta,
                uri,
            } => {
                debug!("Handling RevDelta from editor: {:#?}", rev_delta);
                let file_path = self
                    .file_path_for_uri(uri)
                    .map_err(anyhow_err_to_protocol_err)?;
                if self.ot_servers.get_mut(&file_path).is_none() {
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
                    .get_mut(&file_path)
                    .expect("Could not find OT server.");
                let (delta_for_crdt, rev_deltas_for_editor) =
                    ot_server.apply_editor_operation(rev_delta.clone());

                let messages_to_editor = rev_deltas_for_editor
                    .into_iter()
                    .map(|rev_delta_for_editor| EditorProtocolMessageToEditor::Edit {
                        uri: format!("file://{}", self.absolute_path_for_file_path(&file_path)),
                        delta: rev_delta_for_editor,
                    })
                    .collect();

                Ok((
                    InsideMessage::Edit {
                        file_path,
                        delta: delta_for_crdt,
                    },
                    messages_to_editor,
                ))
            }
            EditorProtocolMessageFromEditor::Cursor { uri, ranges } => {
                let file_path = self
                    .file_path_for_uri(uri)
                    .map_err(anyhow_err_to_protocol_err)?;
                Ok((
                    InsideMessage::Cursor {
                        cursor_id: self.id.clone(),
                        name: env::var("USER").ok(),
                        file_path,
                        ranges: ranges.clone(),
                    },
                    vec![],
                ))
            }
        }
    }

    fn absolute_path_for_file_path(&self, file_path: &str) -> String {
        format!("{}/{}", self.base_dir.display(), file_path)
    }

    fn file_path_for_uri(&self, uri: &str) -> anyhow::Result<String> {
        // If uri starts with "file://", we remove it.
        let absolute_path = uri.strip_prefix("file://").unwrap_or(uri);

        // Check that it's an absolute path.
        if !absolute_path.starts_with('/') {
            bail!("Path '{absolute_path}' is not an absolute file:// URI");
        }

        let base_dir_string = self.base_dir.display().to_string() + "/";

        Ok(absolute_path
            .strip_prefix(&base_dir_string)
            .with_context(|| {
                format!("Path '{absolute_path}' is not within base dir '{base_dir_string}'")
            })?
            .to_string())
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
        let mut editor_connection = EditorConnection::new("1".to_string(), dir.path());

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

        let mut editor_connection = EditorConnection::new("1".to_string(), dir.path());

        // Editor opens the file.
        let result =
            editor_connection.message_from_editor(&EditorProtocolMessageFromEditor::Open {
                uri: format!("file://{}", file.display()),
            });
        assert_eq!(
            result,
            Ok((
                InsideMessage::Open {
                    file_path: "file".to_string()
                },
                vec![]
            ))
        );

        // Daemon sends an edit.
        let delta = insert(1, "x"); // hello -> hxello
        let result = editor_connection.message_from_daemon(&InsideMessage::Edit {
            file_path: "file".to_string(),
            delta,
        });
        assert_eq!(
            result,
            vec![EditorProtocolMessageToEditor::Edit {
                uri: format!("file://{}", file.display()),
                delta: rev_ed_delta(0, ed_delta_single((0, 1), (0, 1), "x"))
            }]
        );

        // Editor sends an edit.
        let delta = rev_ed_delta(0, ed_delta_single((0, 3), (0, 3), "y")); // hello -> helylo
        let result =
            editor_connection.message_from_editor(&EditorProtocolMessageFromEditor::Edit {
                uri: format!("file://{}", file.display()),
                delta,
            });
        let (inside_message, messages_to_editor) = result.unwrap();
        let delta = insert(4, "y"); // Position gets transformed!
        assert_eq!(
            inside_message,
            InsideMessage::Edit {
                file_path: "file".to_string(),
                delta
            }
        );
        assert_eq!(
            messages_to_editor,
            vec![EditorProtocolMessageToEditor::Edit {
                uri: format!("file://{}", file.display()),
                delta: rev_ed_delta(1, ed_delta_single((0, 1), (0, 1), "x")) // Delta is still the
                                                                             // same.
            }]
        );
    }
}
