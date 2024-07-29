//! 👮🚨🚓
//! The functions in this module are supposed to prevent file I/O outside the base directory.
//! All our file I/O should go through them.

use anyhow::{bail, Result};
use path_clean::PathClean;
use std::fs;
use std::path::{Path, PathBuf};

pub fn read_file(absolute_base_dir: &Path, absolute_file_path: &Path) -> Result<Vec<u8>> {
    let canonical_file_path =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_file_path)?;
    let bytes = fs::read(canonical_file_path)?;
    Ok(bytes)
}

pub fn write_file(
    absolute_base_dir: &Path,
    absolute_file_path: &Path,
    content: &[u8],
) -> Result<()> {
    let canonical_file_path =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_file_path)?;
    fs::write(canonical_file_path, content)?;
    Ok(())
}

pub fn remove_file(absolute_base_dir: &Path, absolute_file_path: &Path) -> Result<()> {
    let canonical_file_path =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_file_path)?;
    fs::remove_file(canonical_file_path)?;
    Ok(())
}

pub fn create_dir(absolute_base_dir: &Path, absolute_dir_path: &Path) -> Result<()> {
    let canonical_dir_path =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_dir_path)?;
    std::fs::create_dir(canonical_dir_path)?;
    Ok(())
}

pub fn create_dir_all(absolute_base_dir: &Path, absolute_dir_path: &Path) -> Result<()> {
    let canonical_dir_path =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_dir_path)?;
    std::fs::create_dir_all(canonical_dir_path)?;
    Ok(())
}

pub fn exists(absolute_base_dir: &Path, absolute_file_path: &Path) -> Result<bool> {
    let canonical_file_path =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_file_path)?;
    Ok(canonical_file_path.exists())
}

fn check_inside_base_dir_and_canonicalize(base_dir: &Path, path: &Path) -> Result<PathBuf> {
    let canonical_base_dir = absolute_and_canonicalized(base_dir)?;
    let canonical_path = absolute_and_canonicalized(path)?;

    if !canonical_path.starts_with(canonical_base_dir) {
        bail!("File path is not inside the base directory.");
    }

    Ok(canonical_path)
}

fn absolute_and_canonicalized(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("Path is not absolute.");
    }

    // Remove any ".." and "." from the path.
    let canonical_path = path.clean();

    Ok(canonical_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use temp_dir::TempDir;

    fn temp_dir_setup() -> TempDir {
        let dir = TempDir::new().expect("Failed to create temp directory");
        let project_dir = dir.path().join("project");
        fs::create_dir(&project_dir).expect("Failed to create directory");
        fs::write(project_dir.join("a"), b"This is a file").expect("Failed to write file");
        fs::create_dir(project_dir.join("dir")).expect("Failed to create directory");
        fs::write(project_dir.join("dir").join("b"), b"This is b file")
            .expect("Failed to write file");
        fs::write(dir.path().join("secret"), b"This is a secret").expect("Failed to write file");

        dir
    }

    #[test]
    fn can_read_in_dir() {
        let dir = temp_dir_setup();
        let project_dir = dir.path().join("project");

        assert!(read_file(&project_dir, &project_dir.join("a")).is_ok());
        assert!(read_file(&project_dir, &project_dir.join("dir").join("b")).is_ok());
        assert!(read_file(&project_dir, &project_dir.join("dir").join("..").join("a")).is_ok());
        assert!(read_file(
            &project_dir,
            &project_dir.join(".").join("dir").join(".").join("b")
        )
        .is_ok());
    }

    #[test]
    fn can_not_read_outside_dir() {
        let dir = temp_dir_setup();
        let project_dir = dir.path().join("project");

        // Not a file.
        assert!(read_file(&project_dir, &project_dir).is_err());

        // Not within the base dir.
        assert!(read_file(&project_dir, &project_dir.join("..").join("secret")).is_err());

        // Not a file *and* now within base dir.
        assert!(read_file(&project_dir, &project_dir.join("..")).is_err());

        // Definitely not within base dir.
        assert!(read_file(&project_dir, Path::new("/etc/passwd")).is_err());

        // File path is not absolute.
        assert!(read_file(&project_dir, Path::new("project/a")).is_err());

        // Base dir is not absolute.
        assert!(read_file(Path::new("project"), &project_dir.join("a")).is_err());

        // File not exist.
        assert!(read_file(&project_dir, &project_dir.join("nonexistant")).is_err());
    }
}
