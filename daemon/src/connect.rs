use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tokio::net::UnixListener;

use crate::daemon::DocumentActorHandle;
use crate::editor::spawn_editor_connection;
use tracing::info;

/// # Panics
///
/// Will panic if we fail to listen on the socket, or if we fail to accept an incoming connection.
pub async fn make_editor_connection(socket_path: PathBuf, document_handle: DocumentActorHandle) {
    if Path::new(&socket_path).exists() {
        fs::remove_file(&socket_path).expect("Could not remove/re-initialize socket");
    }
    let result = accept_editor_loop(&socket_path, document_handle).await;
    match result {
        Ok(()) => {}
        Err(err) => {
            panic!("Failed to make editor connection: {err}");
        }
    }
}

async fn accept_editor_loop(
    socket_path: &Path,
    document_handle: DocumentActorHandle,
) -> Result<(), io::Error> {
    let listener = UnixListener::bind(socket_path)?;
    info!("Listening on UNIX socket: {}", socket_path.display());

    loop {
        let (stream, _addr) = listener.accept().await?;
        info!("Editor connection established");

        // TODO: we need to get rid of this await to accept multiple editors.
        spawn_editor_connection(stream, document_handle.clone()).await;
    }
}
