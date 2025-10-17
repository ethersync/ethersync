include!("src/cli.rs");

fn main() -> std::io::Result<()> {
    use clap::{CommandFactory as _, ValueEnum as _};
    const BIN_NAME: &str = env!("CARGO_PKG_NAME");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/cli.rs");

    let target_dir = std::path::Path::new("target");
    let compl_dir = &target_dir.join("completions");
    let man_dir = &target_dir.join("manpages");
    _ = std::fs::remove_dir_all(compl_dir);
    _ = std::fs::remove_dir_all(man_dir);
    std::fs::create_dir_all(compl_dir)?;
    std::fs::create_dir_all(man_dir)?;

    for &shell in clap_complete::Shell::value_variants() {
        clap_complete::generate_to(shell, &mut Cli::command(), BIN_NAME, compl_dir)?;
    }

    clap_mangen::generate_to(Cli::command(), man_dir)?;

    Ok(())
}
