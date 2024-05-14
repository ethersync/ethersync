use nvim_rs::{create::tokio::new_child, rpc::handler::Dummy};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let handler = Dummy::new();

    let (nvim, _, _) = new_child(handler).await.unwrap();
    let buf = nvim.get_current_buf().await.unwrap();

    // repeat 100 times
    for _ in 0..1000 {
        let vim_normal_command = "ie";
        nvim.command(&format!(r#"silent! execute "normal {vim_normal_command}""#))
            //.command_output(&format!(r#"silent! execute "normal {vim_normal_command}""#))
            //.input(vim_normal_command)
            .await
            .expect("Failed to send input to Neovim");
    }

    //sleep(Duration::from_millis(10)).await;

    let line_count = buf.line_count().await.unwrap();
    dbg!(line_count);
    let lines = buf.get_lines(0, line_count, true).await.unwrap();
    dbg!(&lines);
    dbg!(lines[0].len());

    //nvim.command("w! /tmp/test").await.unwrap();

    // TODO: Replace Lua tests with this setup?
}
