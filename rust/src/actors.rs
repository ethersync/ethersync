use crate::types::TextDelta;
use async_trait::async_trait;
use nvim_rs::{compat::tokio::Compat, create::tokio::new_child, rpc::handler::Dummy};
use rand::Rng;
use std::path::PathBuf;
use tokio::process::ChildStdin;

#[async_trait]
pub trait Actor {
    async fn content(&self) -> String;
    async fn apply_delta(&mut self, delta: TextDelta);
    //fn wait_for_sync(&self);
    async fn set_online(&mut self, online: bool);
}

pub struct Daemon {}

pub struct Neovim {
    nvim: nvim_rs::Neovim<Compat<ChildStdin>>,
}

pub struct Buffer {
    buffer: nvim_rs::Buffer<Compat<ChildStdin>>,
}

impl Daemon {
    pub fn new() -> Self {
        todo!()
    }

    pub fn launch(&mut self, _address: Option<String>) {
        todo!()
    }

    pub fn tcp_address(&self) -> String {
        todo!()
    }
}

impl Neovim {
    pub async fn new() -> Self {
        let handler = Dummy::new();

        let (nvim, _, _) = new_child(handler).await.unwrap();

        Self { nvim }
    }

    pub async fn open(&mut self, file: PathBuf) -> Buffer {
        let buffer = self.nvim.get_current_buf().await.unwrap();

        Buffer { buffer }
    }
}

#[async_trait]
impl Actor for Daemon {
    async fn content(&self) -> String {
        todo!()
    }

    async fn apply_delta(&mut self, delta: TextDelta) {
        todo!()
    }

    async fn set_online(&mut self, online: bool) {
        todo!()
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

    async fn apply_delta(&mut self, delta: TextDelta) {
        // TODO: Actually apply the delta.
        self.buffer
            .set_text(0, 0, 0, 0, vec!["!".into()])
            .await
            .unwrap();
    }

    async fn set_online(&mut self, online: bool) {
        todo!()
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
    if rand::thread_rng().gen_range(0.0..0.1) < 0.5 {
        // Insertion.
        let start = rand_range_inclusive(0, content_length);
        let length = rand_range_inclusive(0, 10);
        let text = (0..length)
            .map(|_| rand::random::<char>())
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
        let mut runtime = tokio::runtime::Runtime::new().unwrap();
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
