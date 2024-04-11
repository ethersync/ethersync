use async_trait::async_trait;
use ethersync::daemon::Daemon;
use nvim_rs::{compat::tokio::Compat, create::tokio::new_child_cmd, rpc::handler::Dummy};
use rand::Rng;
use std::path::PathBuf;
use tokio::process::ChildStdin;

// TODO: Consider renaming this, to avoid confusion with tokio "actors".
#[async_trait]
pub trait Actor {
    async fn apply_random_delta(&mut self);
    async fn content(&self) -> String;
    //fn wait_for_sync(&self);
    //async fn set_online(&mut self, online: bool);
}

pub struct Neovim {
    nvim: nvim_rs::Neovim<Compat<ChildStdin>>,
    buffer: nvim_rs::Buffer<Compat<ChildStdin>>,
}

impl Neovim {
    pub async fn new(file_path: PathBuf) -> Self {
        let handler = Dummy::new();
        let mut cmd = tokio::process::Command::new("nvim");
        cmd.arg("--headless").arg("--embed");
        let (nvim, _, _) = new_child_cmd(&mut cmd, handler).await.unwrap();

        nvim.command(&format!("edit! {}", file_path.display()))
            .await
            .expect("Opening file in nvim failed");
        let buffer = nvim.get_current_buf().await.unwrap();

        Self { nvim, buffer }
    }
}

#[async_trait]
impl Actor for Daemon {
    async fn apply_random_delta(&mut self) {
        self.apply_random_delta().await;
    }

    async fn content(&self) -> String {
        self.content().await.unwrap()
    }
}

#[async_trait]
impl Actor for Neovim {
    async fn apply_random_delta(&mut self) {
        let mut vim_normal_command = String::new();

        let directions = ["h", "j", "k", "l"];
        (0..1).for_each(|_| {
            vim_normal_command
                .push_str(directions[rand::thread_rng().gen_range(0..(directions.len()))]);
        });
        vim_normal_command.push('i');
        let vim_components = vec!["x", "🥕", "_", "💚"];
        vim_normal_command.push_str(&random_string(rand_usize_inclusive(0, 10), vim_components));

        //vim_normal_command.push_str("lix");
        //vim_normal_command.push_str("\u{1b}");

        self.nvim
            .command(&format!(r#"execute "normal {vim_normal_command}""#))
            .await
            .expect("Executing normal command failed");
    }
    async fn content(&self) -> String {
        self.buffer
            .get_lines(0, -1, false)
            .await
            .unwrap()
            .join("\n")
    }

    /*
    async fn apply_delta(&mut self, _delta: TextDelta) {
        // TODO: Actually apply the delta.
        self.buffer
            .set_text(0, 0, 0, 0, vec!["!".into()])
            .await
            .unwrap();
    }*/
}

fn random_string(length: usize, components: Vec<&str>) -> String {
    (0..length)
        .map(|_| components[rand_usize_inclusive(0, components.len() - 1)])
        .collect::<String>()
}

fn rand_usize_inclusive(start: usize, end: usize) -> usize {
    if start == end {
        start
    } else {
        rand::thread_rng().gen_range(start..=end)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    #[ignore] // TODO: enable as soon as we have figured out how to install plugin on gh actions
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
}
