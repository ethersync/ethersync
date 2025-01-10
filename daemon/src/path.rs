use anyhow::{bail, Context};
use automerge::Prop;
use derive_more::{AsRef, Deref, Display};
use serde::{Deserialize, Serialize};
use std::path::{self, Path, PathBuf};

/// Paths like these are guaranteed to be absolute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, Hash, Deref, AsRef, Display)]
#[as_ref(Path)]
#[display("'{}'", self.0.display())]
pub struct AbsolutePath(PathBuf);

impl AbsolutePath {
    pub fn from_parts(base: &Path, relative_path: &RelativePath) -> Result<Self, anyhow::Error> {
        let path = base.join(relative_path);
        Self::try_from(path)
    }

    pub fn to_file_uri(&self) -> FileUri {
        FileUri::try_from(format!("file://{}", self.0.display()))
            .expect("Should be able to create File URI from absolute path")
    }
}

impl TryFrom<PathBuf> for AbsolutePath {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        if !path.is_absolute() {
            bail!("Path '{}' is not absolute", path.display());
        }

        Ok(Self(path))
    }
}

impl TryFrom<&str> for AbsolutePath {
    type Error = anyhow::Error;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        Path::new(&path).to_path_buf().try_into()
    }
}

/// Paths like these are relative to the project directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, Hash, Deref, AsRef, Display)]
#[as_ref(Path)]
#[display("'{}'", self.0.display())]
pub struct RelativePath(PathBuf);

impl RelativePath {
    // TODO: This doesn't check the parameter in any way. Should it?
    pub fn new(path: &str) -> Self {
        Self(path.into())
    }

    pub fn try_from_absolute(_base_dir: &Path, _path: &AbsolutePath) -> anyhow::Result<Self> {
        let base_dir;
        let path;

        #[cfg(windows)]
        {
            base_dir = Self::strip_extended_prefix(_base_dir);
            path = Self::strip_extended_prefix(_path);
        }

        #[cfg(unix)]
        {
            base_dir = _base_dir.to_path_buf();
            path = _path.to_path_buf();
        }

        let project_dir = path::absolute(base_dir.clone()).with_context(|| {
            format!(
                "Failed to get absolute path for project directory '{}'",
                base_dir.display()
            )
        })?;
        let relative_path = path.strip_prefix(&project_dir).with_context(|| {
            format!(
                "Failed to strip project directory '{}' from path {_path}",
                project_dir.display()
            )
        })?;

        if relative_path.iter().count() == 0 {
            bail!("base_dir was equal to path when computing relative path");
        }

        Ok(Self(relative_path.to_path_buf()))
    }

    #[cfg(windows)]
    pub fn strip_extended_prefix(p: &Path) -> std::path::PathBuf {
        let s = p.display().to_string();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            // Rebuild as PathBuf
            std::path::PathBuf::from(stripped)
        } else {
            p.to_path_buf()
        }
    }

    pub fn try_from_path(base_dir: &Path, path: &Path) -> Result<Self, anyhow::Error> {
        let absolute_path = AbsolutePath::try_from(path.to_path_buf())?;
        Self::try_from_absolute(base_dir, &absolute_path)
    }
}

impl From<&RelativePath> for Prop {
    fn from(val: &RelativePath) -> Self {
        Prop::Map(val.0.display().to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref)]
pub struct FileUri(String);

impl FileUri {
    pub fn to_absolute_path(&self) -> AbsolutePath {
        #[cfg(unix)]
        let path = Path::new(&self.0[7..]);
        #[cfg(windows)]
        let path = Path::new(&self.0[8..]);
        AbsolutePath::try_from(path.to_path_buf())
            .expect("File URI should contain an absolute path")
    }
}

impl TryFrom<String> for FileUri {
    type Error = anyhow::Error;

    fn try_from(string: String) -> Result<Self, Self::Error> {
        let uri;
        let file_prefix;
        #[cfg(unix)]
        {
            file_prefix = "file:///";
            uri = string;
        }
        #[cfg(windows)]
        {
            file_prefix = "file://";
            uri = string.replace("%3A", ":");
        }
        if uri.starts_with(file_prefix) {
            Ok(Self(uri.to_string()))
        } else {
            bail!("File URI '{}' does not start with '{}'", uri, file_prefix);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_file_path_for_uri_fails_not_absolute() {
        assert!(AbsolutePath::try_from("this/is/absolutely/not/absolute").is_err());
    }

    #[test]
    fn test_file_path_for_uri_fails_not_within_base_dir() {
        let base_dir = Path::new("/an/absolute/path");
        let path = AbsolutePath::try_from("/a/very/different/path").unwrap();

        assert!(RelativePath::try_from_absolute(base_dir, &path,).is_err());
    }

    #[test]
    fn test_file_path_for_uri_fails_not_within_base_dir_suffix() {
        let base_dir = Path::new("/an/absolute/path");
        let path = AbsolutePath::try_from("/an/absolute/path2/file").unwrap();

        assert!(RelativePath::try_from_absolute(base_dir, &path,).is_err());
    }

    #[test]
    fn test_file_path_for_uri_fails_only_base_dir() {
        let base_dir = Path::new("/an/absolute/path");
        let path = AbsolutePath::try_from("/an/absolute/path").unwrap();

        assert!(RelativePath::try_from_absolute(base_dir, &path,).is_err());
    }

    #[test]
    fn test_file_path_for_uri_works() {
        let base_dir = Path::new("/an/absolute/path");

        let file_paths = vec!["file1", "sub/file3", "sub"];
        for &expected in &file_paths {
            let uri =
                FileUri::try_from(format!("file://{}/{}", base_dir.display(), expected)).unwrap();
            let absolute_path = uri.to_absolute_path();
            let relative_path = RelativePath::try_from_absolute(base_dir, &absolute_path).unwrap();

            assert_eq!(RelativePath::new(expected), relative_path);
        }
    }
}
