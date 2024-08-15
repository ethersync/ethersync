use ethersync::daemon::Daemon;
use ethersync::sandbox;

use async_trait::async_trait;
pub use nvim_rs::{compat::tokio::Compat, create::tokio::new_child_cmd, rpc::handler::Dummy};
use rand::Rng;
use temp_dir::TempDir;
use tokio::process::ChildStdin;

use std::fs;
use std::path::PathBuf;

// TODO: Consider renaming this, to avoid confusion with tokio "actors".
#[async_trait]
pub trait Actor: Send {
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
    /// # Panics
    ///
    /// Will panic if Neovim cannot be started or if the file cannot be opened.
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

    /// # Panics
    ///
    /// Will panic if input cannot be sent to Neovim.
    pub async fn input(&mut self, input: &str) {
        self.nvim
            .input(input)
            .await
            .expect("Failed to send input to Neovim");
    }
}

#[async_trait]
impl Actor for Daemon {
    async fn apply_random_delta(&mut self) {
        self.document_handle.apply_random_delta().await;
    }

    async fn content(&self) -> String {
        self.document_handle.content().await.unwrap()
    }
}

#[async_trait]
impl Actor for Neovim {
    async fn apply_random_delta(&mut self) {
        let mut vim_normal_command = String::new();

        let string_components = vec![
            "e".to_string(),
            "ä".to_string(),
            "💚".to_string(),
            "🥕".to_string(),
            "\n".to_string(),
        ];
        let s = random_string(rand_usize_inclusive(1, 5), &string_components);

        let components = vec![
            "h".to_string(),
            "j".to_string(),
            "k".to_string(),
            "l".to_string(),
            "gg".to_string(),
            "G".to_string(),
            "$".to_string(),
            "^".to_string(),
            "x".to_string(),
            "vllld".to_string(),
            "Vjjjd".to_string(),
            "rü".to_string(),
            "dd".to_string(),
            "J".to_string(),
            format!("i{}", s),
            format!("jjI{}", s),
            format!("o{}", s),
            format!("O{}", s),
            format!("A{}", s),
            format!("I{}", s),
        ];

        vim_normal_command.push_str(&random_string(rand_usize_inclusive(1, 2), &components));

        self.nvim
            .command(&format!(r#"silent! execute "normal {vim_normal_command}""#))
            //.input(&vim_normal_command)
            .await
            .expect("Failed to send input to Neovim");
    }
    async fn content(&self) -> String {
        let mut content = self
            .buffer
            .get_lines(0, -1, false)
            .await
            .unwrap()
            .join("\n");
        if self
            .nvim
            .command_output("set eol?")
            .await
            .expect("Failed to get value of eol")
            .trim()
            == "endofline"
        {
            content.push('\n');
        }
        content
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

fn random_string(length: usize, components: &[String]) -> String {
    (0..length)
        .map(|_| components[rand_usize_inclusive(0, components.len() - 1)].clone())
        .collect::<String>()
}

fn rand_usize_inclusive(start: usize, end: usize) -> usize {
    if start == end {
        start
    } else {
        rand::thread_rng().gen_range(start..=end)
    }
}

impl Neovim {
    pub async fn new_ethersync_enabled(initial_content: &str) -> (Self, PathBuf) {
        let dir = TempDir::new().unwrap();
        let ethersync_dir = dir.child(".ethersync");
        sandbox::create_dir(dir.path(), &ethersync_dir).unwrap();
        let mut file_path = dir.child("test");
        sandbox::write_file(dir.path(), &file_path, initial_content.as_bytes())
            .expect("Failed to write initial file content");
        file_path = fs::canonicalize(file_path).expect("Could not canonicalize");

        (Self::new(file_path.clone()).await, file_path)
    }
}