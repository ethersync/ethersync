// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{config, ot::OTServer, sandbox};
use anyhow::bail;
use derive_more::Deref;
use std::{collections::HashMap, path::PathBuf};
use tracing::debug;
use url::Url;

use ethersync_shared::{
    path::{AbsolutePath, RelativePath},
    types::{
        ComponentMessage, CursorState, EditorProtocolMessageError, EditorProtocolMessageFromEditor,
        EditorProtocolMessageToEditor, RevisionedEditorTextDelta,
    },
};

// TODO: Wrap the newtype around url::Url instead?
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref)]
#[must_use]
pub struct FileUri(String);

impl FileUri {
    pub fn from_absolute_path(path: &AbsolutePath) -> Self {
        Self::try_from(format!("file://{}", path.display()))
            .expect("Should be able to create File URI from absolute path")
    }

    pub fn to_absolute_path(&self) -> AbsolutePath {
        let path_buf = Url::parse(self)
            .expect("Should be able to parse file:// URL as Url")
            .to_file_path()
            .expect("Should be able to convert Url to PathBuf");
        AbsolutePath::try_from(path_buf).expect("File URI should contain an absolute path")
    }
}

impl TryFrom<String> for FileUri {
    type Error = anyhow::Error;

    fn try_from(string: String) -> Result<Self, Self::Error> {
        // TODO: Could be written simpler?
        if string.starts_with("file:///") {
            // Use the url crate to properly URL encode the path (spaces should be "%20", for example).
            Ok(Self(
                Url::parse(&string)
                    .expect("Should be able to parse file:// URL")
                    .to_string(),
            ))
        } else {
            bail!("File URI '{}' does not start with 'file:///'", string);
        }
    }
}

/// Represents a connection to an editor. Handles the OT. To keep the code testable and sync, we do
/// the actual sending of messages in the daemon, and the functions here just *calculate* them.
#[must_use]
pub struct EditorConnection {
    id: String,
    // TODO: Feels a bit duplicated here?
    base_dir: PathBuf,
    /// There's one [`OTServer`] per open buffer.
    ot_servers: HashMap<RelativePath, OTServer>,
    /// The name other people see.
    username: Option<String>,
}

impl EditorConnection {
    pub fn new(id: String, base_dir: PathBuf) -> Self {
        Self {
            id,
            username: config::get_username(&base_dir),
            base_dir,
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

                    let self1 = &AbsolutePath::from_parts(&self.base_dir, file_path)
                        .expect("Should be able to construct absolute URI");
                    let uri = FileUri::from_absolute_path(self1);

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
                let self1 = &AbsolutePath::from_parts(&self.base_dir, &cursor_state.file_path)
                    .expect("Should be able to construct absolute URI");
                let uri = FileUri::from_absolute_path(self1);

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
                let relative_path = RelativePath::try_from_absolute(&self.base_dir, &absolute_path)
                    .map_err(anyhow_err_to_protocol_err)?;

                debug!("Got an 'open' message for {relative_path}");
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
                let relative_path = RelativePath::try_from_absolute(&self.base_dir, &absolute_path)
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
                let relative_path = RelativePath::try_from_absolute(&self.base_dir, &absolute_path)
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

                let self1 = &AbsolutePath::from_parts(&self.base_dir, &relative_path)
                    .expect("Should be able to construct absolute URI");
                let uri = FileUri::from_absolute_path(self1);

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
                let relative_path = RelativePath::try_from_absolute(&self.base_dir, &absolute_path)
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
    use ethersync_shared::types::factories::*;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use temp_dir::TempDir;

    #[test]
    fn opening_file_in_wrong_dir_fails() {
        let dir = TempDir::new().expect("Failed to create temp directory");
        let mut editor_connection =
            EditorConnection::new("1".to_string(), dir.path().to_path_buf());

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

        let mut editor_connection =
            EditorConnection::new("1".to_string(), dir.path().to_path_buf());

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

    #[test]
    fn test_file_path_for_uri_works() {
        let base_dir = Path::new("/an/absolute/path");

        let file_paths = vec!["file1", "sub/file3", "sub"];
        for &expected in &file_paths {
            let uri =
                FileUri::try_from(format!("file://{}/{}", base_dir.display(), expected)).unwrap();
            let absolute_path = uri.to_absolute_path();
            let relative_path = RelativePath::try_from_absolute(base_dir, &absolute_path).unwrap();

            assert_eq!(RelativePath::new(expected), relative_path);
        }
    }

    #[test]
    fn test_uri_encoding_works_with_spaces() {
        let uri = FileUri::try_from("file:///a/b/file with spaces".to_string()).unwrap();
        assert_eq!("file:///a/b/file%20with%20spaces", uri.0);
    }

    #[test]
    fn test_uri_decoding_works_with_spaces() {
        let uri = FileUri::try_from("file:///a/b/file with spaces".to_string()).unwrap();
        let absolute_path = AbsolutePath::try_from("/a/b/file with spaces").unwrap();
        assert_eq!(absolute_path, uri.to_absolute_path());
    }
}
