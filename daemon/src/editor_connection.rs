// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use tracing::debug;

use crate::{
    config::{self, AppConfig},
    editor_protocol::{
        EditorProtocolMessageError, EditorProtocolMessageFromEditor, EditorProtocolMessageToEditor,
    },
    ot::OTServer,
    path::{AbsolutePath, FileUri, RelativePath},
    sandbox,
    types::{ComponentMessage, CursorState, RevisionedEditorTextDelta},
};

/// Represents a connection to an editor. Handles the OT. To keep the code testable and sync, we do
/// the actual sending of messages in the daemon, and the functions here just *calculate* them.
#[must_use]
pub struct EditorConnection {
    id: String,
    // TODO: Feels duplicated here?
    app_config: AppConfig,
    /// There's one [`OTServer`] per open buffer.
    ot_servers: HashMap<RelativePath, OTServer>,
    /// The name other people see.
    username: Option<String>,
}

impl EditorConnection {
    pub fn new(id: String, app_config: AppConfig) -> Self {
        Self {
            id,
            username: config::get_username(&app_config.base_dir),
            app_config,
            ot_servers: HashMap::new(),
        }
    }

    #[must_use]
    pub fn owns(&self, file_path: &RelativePath) -> bool {
        self.ot_servers.contains_key(file_path)
    }

    /// A message from inside is either an edit from another local editor or an edit that came
    /// from another peer but is prepared to be applied to all components.
    #[must_use]
    pub fn message_from_inside(
        &mut self,
        message: &ComponentMessage,
    ) -> Vec<EditorProtocolMessageToEditor> {
        match message {
            ComponentMessage::Edit { file_path, delta } => {
                if let Some(ot_server) = self.ot_servers.get_mut(file_path) {
                    debug!("Applying incoming CRDT patch for {file_path}");
                    let rev_text_delta_for_editor = ot_server.apply_crdt_change(delta);

                    let uri = AbsolutePath::from_parts(&self.app_config.base_dir, file_path)
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
                cursor_id,
                cursor_state,
            } => {
                let uri =
                    AbsolutePath::from_parts(&self.app_config.base_dir, &cursor_state.file_path)
                        .expect("Should be able to construct absolute URI")
                        .to_file_uri();

                vec![EditorProtocolMessageToEditor::Cursor {
                    userid: cursor_id.clone(),
                    name: cursor_state.name.clone(),
                    uri: uri.to_string(),
                    ranges: cursor_state.ranges.clone(),
                }]
            }
            _ => {
                debug!("Ignoring message from inside: {message:#?}");
                vec![]
            }
        }
    }

    /// When processing an edit, this method will return edits that should be sent to the editor.
    /// These edits that are returned are transformed edits, which take into account what the
    /// editor has missed.
    pub fn message_from_editor(
        &mut self,
        message: &EditorProtocolMessageFromEditor,
    ) -> Result<(ComponentMessage, Vec<EditorProtocolMessageToEditor>), EditorProtocolMessageError>
    {
        #[expect(clippy::needless_pass_by_value)] // map_err takes by value
        fn anyhow_err_to_protocol_err(error: anyhow::Error) -> EditorProtocolMessageError {
            EditorProtocolMessageError {
                code: -1, // TODO: Should the error codes differ per error?
                message: error.to_string(),
                data: None,
            }
        }

        match message {
            EditorProtocolMessageFromEditor::Open { uri, content } => {
                let uri = FileUri::try_from(uri.clone()).map_err(anyhow_err_to_protocol_err)?;
                let absolute_path = uri.to_absolute_path();
                let relative_path =
                    RelativePath::try_from_absolute(&self.app_config.base_dir, &absolute_path)
                        .map_err(anyhow_err_to_protocol_err)?;

                debug!("Got an 'open' message for {relative_path}");
                if !sandbox::exists(&self.app_config.base_dir, &absolute_path)
                    .map_err(anyhow_err_to_protocol_err)?
                {
                    // Creating nonexisting files allows us to traverse this file for whether it's
                    // ignored, which is needed to even be allowed to open it.
                    sandbox::write_file(&self.app_config.base_dir, &absolute_path, b"")
                        .map_err(anyhow_err_to_protocol_err)?;
                }

                // We only want to process these messages for files that are not ignored.
                if sandbox::ignored(&self.app_config, &absolute_path)
                    .expect("Could not check ignore status of opened file")
                {
                    return Err(EditorProtocolMessageError {
                        code: -1,
                        message: format!("File {absolute_path} is ignored"),
                        data: Some("This file should not be shared with other peers".into()),
                    });
                }

                let ot_server = OTServer::new(content.clone());
                self.ot_servers.insert(relative_path.clone(), ot_server);

                Ok((
                    ComponentMessage::Open {
                        file_path: relative_path,
                        content: content.clone(),
                    },
                    vec![],
                ))
            }
            EditorProtocolMessageFromEditor::Close { uri } => {
                let uri = FileUri::try_from(uri.clone()).map_err(anyhow_err_to_protocol_err)?;
                let absolute_path = uri.to_absolute_path();
                let relative_path =
                    RelativePath::try_from_absolute(&self.app_config.base_dir, &absolute_path)
                        .map_err(anyhow_err_to_protocol_err)?;

                debug!("Got a 'close' message for {relative_path}");
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
                let relative_path =
                    RelativePath::try_from_absolute(&self.app_config.base_dir, &absolute_path)
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
                    ot_server.apply_editor_operation(rev_delta);

                let uri = AbsolutePath::from_parts(&self.app_config.base_dir, &relative_path)
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
                let relative_path =
                    RelativePath::try_from_absolute(&self.app_config.base_dir, &absolute_path)
                        .map_err(anyhow_err_to_protocol_err)?;

                Ok((
                    ComponentMessage::Cursor {
                        cursor_id: self.id.clone(),
                        cursor_state: CursorState {
                            name: self.username.clone(),
                            file_path: relative_path,
                            ranges: ranges.clone(),
                        },
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

        let app_config = AppConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut editor_connection = EditorConnection::new("1".to_string(), app_config);

        let result =
            editor_connection.message_from_editor(&EditorProtocolMessageFromEditor::Open {
                uri: "file:///foobar/file".to_string(),
                content: String::new(),
            });

        assert!(result.is_err());
    }

    #[test]
    fn edits_are_oted() {
        let dir = TempDir::new().expect("Failed to create temp directory");
        let file = dir.path().join("file");
        std::fs::write(&file, "hello").expect("Failed to write file");

        let app_config = AppConfig {
            base_dir: dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut editor_connection = EditorConnection::new("1".to_string(), app_config);

        // Editor opens the file.
        let result =
            editor_connection.message_from_editor(&EditorProtocolMessageFromEditor::Open {
                uri: format!("file://{}", file.display()),
                content: "initial content".to_string(),
            });
        assert_eq!(
            result,
            Ok((
                ComponentMessage::Open {
                    file_path: RelativePath::new("file"),
                    content: "initial content".to_string(),
                },
                vec![]
            ))
        );

        // Daemon sends an edit.
        let delta = insert(1, "x"); // hello -> hxello
        let result = editor_connection.message_from_inside(&ComponentMessage::Edit {
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
