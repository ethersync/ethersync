use nvim_rs::{create::tokio::new_child, rpc::handler::Dummy};

#[tokio::main]
async fn main() {
    let handler = Dummy::new();

    let (nvim, _, _) = new_child(handler).await.unwrap();
    let buf = nvim.get_current_buf().await.unwrap();

    buf.set_text(0, 0, 0, 0, vec!["hello world".into()])
        .await
        .unwrap();

    let line_count = buf.line_count().await.unwrap();
    dbg!(line_count);
    let lines = buf.get_lines(0, line_count, true).await.unwrap();
    dbg!(lines);

    nvim.command("w! /tmp/test").await.unwrap();

    // TODO: Replace Lua tests with this setup?
}
