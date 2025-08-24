use crate::cli::ask;
use crate::daemon::{DocMessage, DocumentActorHandle};
use crate::editor::{
    handle_editor_connection, Editor, EditorId, EditorProtocolCodec, EditorStream,
};
use crate::sandbox;
use anyhow::{bail, Context};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio::net::UnixListener;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};
use tracing::{debug, info};

pub struct EditorUnix {
    pub socket_path: PathBuf,
}

impl Editor for EditorUnix {
    fn get_socket_path(&self) -> PathBuf {
        self.socket_path.clone()
    }

    /// # Panics
    ///
    /// Will panic if we fail to listen on the socket, or if we fail to accept an incoming connection.
    fn spawn_socket_listener(
        &self,
        document_handle: DocumentActorHandle,
    ) -> anyhow::Result<()> {
        // Make sure the parent directory of the socket is only accessible by the current user.
        if let Err(description) = is_user_readable_only(&self.socket_path) {
            panic!("{}", description);
        }

        // Using the sandbox method here is technically unnecessary,
        // but we want to really run all path operations through the sandbox module.
        // TODO: Use correct directory as guard.
        if sandbox::exists(Path::new("/"), Path::new(&self.socket_path))
            .expect("Failed to check existence of path")
        {
            let socket_path_display = self.socket_path.display();
            let remove_socket = ask(&format!("Detected an existing socket '{socket_path_display}'. There might be a daemon running already for this directory, or the previous one crashed. Do you want to continue?"));
            if remove_socket? {
                sandbox::remove_file(Path::new("/"), &self.socket_path)
                    .expect("Could not remove socket");
            } else {
                bail!("Not continuing, make sure to stop all other daemons on this directory");
            }
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        debug!("Listening on UNIX socket: {}", self.socket_path.display());

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        let id = document_handle.clone().next_editor_id();
                        let document_handle_clone = document_handle.clone();
                        tokio::spawn(async move {
                            handle_editor_connection(stream, document_handle_clone.clone(), id)
                                .await;
                        })
                    }
                    Err(err) => {
                        panic!("Error while accepting socket connection: {err}");
                    }
                };
            }
        });

        Ok(())
    }
}

fn is_user_readable_only(socket_path: &Path) -> anyhow::Result<()> {
    let parent_dir = socket_path
        .parent()
        .context("The socket path should not be the root directory")?;
    let current_permissions = fs::metadata(parent_dir)
        .context("Expected to have access to metadata of the socket path's parent")?
        .permissions()
        .mode();
    // Group and others should not have any permissions.
    let allowed_permissions = 0o77700u32;
    if current_permissions | allowed_permissions != allowed_permissions {
        bail!("For security reasons, the parent directory of the socket must only be accessible by the current user. Please run `chmod go-rwx {:?}`", parent_dir);
    }
    Ok(())
}
