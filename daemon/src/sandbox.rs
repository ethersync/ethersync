// SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
// SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 👮🚨🚓
//! The functions in this module are supposed to prevent file I/O outside the base directory.
//! All our file I/O should go through them.

use anyhow::{bail, Context, Result};
use ignore::WalkBuilder;
use path_clean::PathClean;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn read_file(absolute_base_dir: &Path, absolute_file_path: &Path) -> Result<Vec<u8>> {
    let canonical_file_path =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_file_path)?;
    let bytes = fs::read(canonical_file_path)?;
    Ok(bytes)
}

/// Writes content to a file, creating the parent directories, if they don't exist.
pub fn write_file(
    absolute_base_dir: &Path,
    absolute_file_path: &Path,
    content: &[u8],
) -> Result<()> {
    let canonical_file_path =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_file_path)?;

    // Create the parent directorie(s), if neccessary.
    let parent_dir = canonical_file_path
        .parent()
        .expect("Failed to get parent directory");
    create_dir_all(absolute_base_dir, parent_dir).expect("Failed to create parent directory");

    fs::write(canonical_file_path, content)?;
    Ok(())
}

pub fn append_file(
    absolute_base_dir: &Path,
    absolute_file_path: &Path,
    content: &[u8],
) -> Result<()> {
    let canonical_file_path =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_file_path)?;
    let mut file = OpenOptions::new().append(true).open(canonical_file_path)?;
    file.write_all(content)?;
    Ok(())
}

pub fn rename_file(
    absolute_base_dir: &Path,
    absolute_file_path_old: &Path,
    absolute_file_path_new: &Path,
) -> Result<()> {
    let canonical_file_path_old =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_file_path_old)?;
    let canonical_file_path_new =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_file_path_new)?;
    fs::rename(canonical_file_path_old, canonical_file_path_new)?;
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
    std::fs::create_dir(&canonical_dir_path)?;
    #[cfg(unix)]
    {
        let permissions = fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(canonical_dir_path, permissions)?;
    }
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

// TODO: Don't build the list of ignored files on every call.
// TODO: Allow calling this for non-existing files.
pub fn ignored(absolute_base_dir: &Path, absolute_file_path: &Path) -> Result<bool> {
    let canonical_file_path =
        check_inside_base_dir_and_canonicalize(absolute_base_dir, absolute_file_path)?;

    let ignored_things = [".git", ".ethersync"];

    // To use the same logic for which files are ignored, iterate through all files
    // using ignore::Walk, and try to find this file.
    // This has the downside that the file must already exist.
    let walk = WalkBuilder::new(absolute_base_dir)
        .standard_filters(true)
        .hidden(false)
        .require_git(false)
        // Interestingly, the standard filters don't seem to ignore .git.
        .filter_entry(move |dir_entry| {
            let name = dir_entry
                .path()
                .file_name()
                .expect("Failed to get file name from path.")
                .to_str()
                .expect("Failed to convert OsStr to str");
            !ignored_things.contains(&name) && !name.ends_with('~')
        })
        .build();

    return Ok(!walk
        .filter_map(Result::ok)
        .filter(|dir_entry| {
            dir_entry
                .file_type()
                .expect("Couldn't get file type of dir entry")
                .is_file()
        })
        .map(|dir_entry| absolute_and_canonicalized(dir_entry.path()))
        .filter_map(Result::ok)
        .any(|path| path == canonical_file_path));
}

fn check_inside_base_dir_and_canonicalize(base_dir: &Path, path: &Path) -> Result<PathBuf> {
    let canonical_base_dir = absolute_and_canonicalized(base_dir)?;
    let canonical_path = absolute_and_canonicalized(path)?;

    if !canonical_path.starts_with(&canonical_base_dir) {
        let canonical_path_str = &canonical_path.display();
        let canonical_base_dir_str = &canonical_base_dir.display();
        bail!("File path {canonical_path_str} is not inside the base directory {canonical_base_dir_str}");
    }

    Ok(canonical_path)
}

fn absolute_and_canonicalized(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("Path is not absolute.");
    }

    // Remove any ".." and "." from the path.
    let canonical_path = path.clean();
    let mut suffix_path = PathBuf::new();
    let mut prefix_path = canonical_path.clone();

    for component in path.components().rev() {
        if prefix_path.exists() {
            break;
        }
        prefix_path.pop();
        if let std::path::Component::Normal(os_str) = component {
            suffix_path = if suffix_path.components().count() != 0 {
                Path::new(os_str).join(&suffix_path)
            } else {
                Path::new(os_str).to_path_buf()
            };
        } else {
            panic!("Got unexpected Component variant while canonicalizing");
        }
    }

    let mut canonical_path = prefix_path
        .canonicalize()
        .context("Failed to canonicalize path, probably the file disappeared already")?;

    if suffix_path.components().count() != 0 {
        canonical_path = canonical_path.join(suffix_path);
    }

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
    #[cfg(unix)]
    fn does_canonicalize_symlink_dir() {
        let dir = temp_dir_setup();
        let linked_project = dir.child("ln_project");
        let project = dir.child("project");
        std::os::unix::fs::symlink(&project, &linked_project).unwrap();
        assert_eq!(
            absolute_and_canonicalized(&linked_project).unwrap(),
            project.canonicalize().unwrap()
        );
    }

    #[test]
    #[cfg(unix)]
    fn does_canonicalize_symlink_file() {
        let dir = temp_dir_setup();
        let linked_project = dir.child("ln_project");
        let project = dir.child("project");
        std::os::unix::fs::symlink(&project, &linked_project).unwrap();

        let ln_file = dir.child("ln_project/c");

        assert_eq!(
            absolute_and_canonicalized(&ln_file).unwrap().to_str(),
            project.canonicalize().unwrap().join("c").to_str()
        );
    }

    #[test]
    #[cfg(unix)]
    fn does_canonicalize_symlink_notexisting_file() {
        let dir = temp_dir_setup();
        let linked_project = dir.child("ln_project");
        let project = dir.child("project");
        std::os::unix::fs::symlink(&project, &linked_project).unwrap();

        let file = dir.child("project/a");
        let ln_file = dir.child("ln_project/a");

        // tests whether it does not end on slash
        assert_eq!(
            absolute_and_canonicalized(&file).unwrap().to_str(),
            file.canonicalize().unwrap().to_str()
        );

        assert_eq!(
            absolute_and_canonicalized(&ln_file).unwrap().to_str(),
            ln_file.canonicalize().unwrap().to_str()
        );
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

    #[test]
    fn fail_check_inside_base_dir() {
        let dir = temp_dir_setup();
        let project_dir = dir.path().join("project");

        // Not within the base dir.
        assert!(read_file(&project_dir, &project_dir.join("..").join("secret")).is_err());

        // It "starts" with the base dir, but it's not inside it.
        assert!(check_inside_base_dir_and_canonicalize(
            &project_dir,
            Path::new(&format!(
                "{}{}",
                project_dir.as_path().to_str().unwrap(),
                "2/file"
            ))
        )
        .is_err());
    }
}
