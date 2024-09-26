use anyhow::{bail, Context};
use automerge::Prop;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fmt::{Display, Formatter},
    path::{self, Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub struct AbsolutePath(String);

impl TryFrom<PathBuf> for AbsolutePath {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        if !path.is_absolute() {
            bail!("Path '{:?}' is not absolute", path);
        }

        if let Some(path) = path.to_str() {
            Ok(Self(path.to_string()))
        } else {
            bail!("Failed to convert Path '{:?}' to string", path);
        }
    }
}

impl From<AbsolutePath> for PathBuf {
    fn from(val: AbsolutePath) -> Self {
        val.0.into()
    }
}

impl Display for AbsolutePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<OsStr> for AbsolutePath {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

/// Paths like these are relative to the project directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub struct RelativePath(pub String);

impl Display for RelativePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl RelativePath {
    pub fn try_from_absolute(path: AbsolutePath, base_dir: &Path) -> Result<Self, anyhow::Error> {
        let path = PathBuf::from(path.0);
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
        Ok(Self(relative_path.to_string_lossy().to_string()))
    }

    pub fn display(&self) -> String {
        self.0.clone()
    }
}

impl From<&RelativePath> for Prop {
    fn from(val: &RelativePath) -> Self {
        Prop::Map(val.0.clone())
    }
}
