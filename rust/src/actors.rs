use async_trait::async_trait;
use ethersync::daemon::Daemon;
use ethersync::types::TextDelta;
use nvim_rs::{compat::tokio::Compat, create::tokio::new_child_cmd, rpc::handler::Dummy};
use rand::Rng;
use std::path::PathBuf;
use tokio::process::ChildStdin;

// TODO: Consider renaming this, to avoid confusion with tokio "actors".
#[async_trait]
pub trait Actor {
    async fn content(&self) -> String;
    async fn apply_delta(&mut self, delta: TextDelta);
    //fn wait_for_sync(&self);
    //async fn set_online(&mut self, online: bool);
}

pub struct Neovim {
    nvim: nvim_rs::Neovim<Compat<ChildStdin>>,
}

pub struct Buffer {
    buffer: nvim_rs::Buffer<Compat<ChildStdin>>,
}

impl Neovim {
    pub async fn new() -> Self {
        let handler = Dummy::new();
        let mut cmd = tokio::process::Command::new("nvim");
        cmd.arg("--headless").arg("--embed");
        let (nvim, _, _) = new_child_cmd(&mut cmd, handler).await.unwrap();

        Self { nvim }
    }

    pub async fn open(&mut self, file_path: PathBuf) -> Buffer {
        self.nvim
            .command(&format!("edit! {}", file_path.display()))
            .await
            .expect("Opening file in nvim failed");
        let buffer = self.nvim.get_current_buf().await.unwrap();

        Buffer { buffer }
    }
}

#[async_trait]
impl Actor for Daemon {
    async fn content(&self) -> String {
        self.content()
            .await
            .expect("Document doesn't have content yet")
    }

    async fn apply_delta(&mut self, delta: TextDelta) {
        self.apply_delta(delta).await;
    }
}

#[async_trait]
impl Actor for Buffer {
    async fn content(&self) -> String {
        self.buffer
            .get_lines(0, -1, false)
            .await
            .unwrap()
            .join("\n")
    }

    async fn apply_delta(&mut self, _delta: TextDelta) {
        // TODO: Actually apply the delta.
        self.buffer
            .set_text(0, 0, 0, 0, vec!["!".into()])
            .await
            .unwrap();
    }
}

// Construct a random insertion or deletion for the content.
pub fn random_delta(content: &str) -> TextDelta {
    fn rand_range_inclusive(start: usize, end: usize) -> usize {
        if start == end {
            start
        } else {
            rand::thread_rng().gen_range(start..=end)
        }
    }

    let content_length = content.chars().count();
    if rand::thread_rng().gen_range(0.0..1.0) < 0.5 {
        // Insertion.
        let start = rand_range_inclusive(0, content_length);
        let number_of_components = rand_range_inclusive(0, 10);
        let components = ["x", "🥕", "_", "💚", "\n"];
        let text = (0..number_of_components)
            .map(|_| components[rand_range_inclusive(0, components.len() - 1)])
            .collect::<String>();
        let mut delta = TextDelta::default();
        delta.retain(start);
        delta.insert(&text);
        delta
    } else {
        // Deletion.
        let start = rand_range_inclusive(0, content_length);
        let length = rand_range_inclusive(0, content_length - start);
        let mut delta = TextDelta::default();
        delta.retain(start);
        delta.delete(length);
        delta
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn buffer_content() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let mut neovim = Neovim::new().await;
            let temp_file = std::env::temp_dir().join("test");
            let mut buffer = neovim.open(temp_file).await;
            let mut delta = TextDelta::default();
            delta.insert("!");
            buffer.apply_delta(delta).await;
            assert_eq!(buffer.content().await, "!");
        });
    }

    #[test]
    fn plugin_loaded() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let handler = Dummy::new();
            let mut cmd = tokio::process::Command::new("nvim");
            cmd.arg("--headless").arg("--embed");
            let (nvim, _, _) = new_child_cmd(&mut cmd, handler).await.unwrap();
            // Test if Ethersync can be run successfully (empty string means the command exists).
            assert_eq!(nvim.command_output("Ethersync").await.unwrap(), "");
        });
    }

    #[test]
    fn test_random_delta() {
        (0..10).for_each(|length| {
            (0..100).for_each(|_| {
                let content = (0..length)
                    .map(|_| rand::random::<char>())
                    .collect::<String>();
                let delta = random_delta(&content);
                // Verify that delta can be applied to content.
                delta.apply(&content);
            });
        });
    }
}
