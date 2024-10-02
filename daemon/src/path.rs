use anyhow::{bail, Context};
use automerge::Prop;
use derive_more::{AsRef, Deref};
use serde::{Deserialize, Serialize};
use std::path::{self, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, Hash, Deref, AsRef)]
#[as_ref(Path)]
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
            bail!("Path '{:?}' is not absolute", path);
        }

        Ok(Self(path))
    }
}

/// Paths like these are relative to the project directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, Hash, Deref, AsRef)]
#[as_ref(Path)]
pub struct RelativePath(PathBuf);

impl RelativePath {
    // TODO: This doesn't check the parameter in any way. Should it?
    pub fn new(path: &str) -> Self {
        Self(path.into())
    }

    pub fn try_from_absolute(path: &AbsolutePath, base_dir: &Path) -> Result<Self, anyhow::Error> {
        let project_dir = path::absolute(base_dir).with_context(|| {
            format!(
                "Failed to get absolute path for project directory '{:?}'",
                base_dir
            )
        })?;
        let relative_path = path.strip_prefix(&project_dir).with_context(|| {
            format!(
                "Failed to strip project directory '{:?}' from path '{:?}'",
                project_dir, path
            )
        })?;
        Ok(Self(relative_path.to_path_buf()))
    }

    pub fn try_from_path(path: &Path, base_dir: &Path) -> Result<Self, anyhow::Error> {
        let absolute_path = AbsolutePath::try_from(path.to_path_buf())?;
        Self::try_from_absolute(&absolute_path, base_dir)
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
        let path = Path::new(&self.0[7..]);
        AbsolutePath::try_from(path.to_path_buf())
            .expect("File URI should contain an absolute path")
    }
}

impl TryFrom<String> for FileUri {
    type Error = anyhow::Error;

    fn try_from(string: String) -> Result<Self, Self::Error> {
        if string.starts_with("file:///") {
            Ok(Self(string.to_string()))
        } else {
            bail!("File URI '{}' does not start with 'file:///'", string);
        }
    }
}
