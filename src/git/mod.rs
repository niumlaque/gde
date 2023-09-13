mod error;
mod gitcheckout;
mod gitdiff;
mod gitlog;
mod gitlstree;
mod onelinelog;

pub use error::{Error, Result};
pub use gitcheckout::GitCheckout;
pub use gitdiff::GitDiff;
pub use gitlog::GitLog;
pub use gitlstree::GitLsTree;
pub use onelinelog::{Commit, OnelineLog};

use std::env::{self, current_dir};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct Git {
    version: String,
    pub(super) path: PathBuf,
}

impl Git {
    pub fn get_version(path: impl AsRef<Path>) -> Result<String> {
        let output = Command::new(path.as_ref())
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        let stdout = String::from_utf8(output.stdout)?;
        let stderr = String::from_utf8(output.stderr)?;

        if !stderr.is_empty() {
            return Err(Error::Command("Failed to get version".into()));
        }

        if let Some(ret) = stdout.split('\n').next() {
            let s = "git version ";
            if ret.starts_with(s) {
                Ok(ret
                    .get(s.len()..ret.len())
                    .map(|x| x.to_string())
                    .unwrap_or(ret.into()))
            } else {
                Ok(ret.into())
            }
        } else {
            Err(Error::Command("Failed to get version".into()))
        }
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let version = Self::get_version(path)?;
        Ok(Self {
            version,
            path: path.into(),
        })
    }

    pub(super) fn exec<R, F: FnOnce(&PathBuf) -> Result<R>>(
        &self,
        target_dir: impl AsRef<Path>,
        f: F,
    ) -> Result<R> {
        let dir = Self::change_currentdir(target_dir)?;
        let ret = f(&self.path);
        Self::change_currentdir(dir)?;
        ret
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn get_rootdir(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        self.exec(path, |git| {
            let output = Command::new(git)
                .arg("rev-parse")
                .arg("--show-superproject-working-tree")
                .arg("--show-toplevel")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;
            let stdout = String::from_utf8(output.stdout)?;

            if !output.status.success() {
                let stderr = String::from_utf8(output.stderr)?;
                println!("{stderr}");
                return Err(Error::Command("Failed to get root directory".into()));
            }

            if let Some(ret) = stdout.split('\n').next() {
                Ok(ret.into())
            } else {
                Err(Error::Command("Failed to get root directory".into()))
            }
        })
    }

    pub fn get_hash(&self, path: impl AsRef<Path>, commit: impl AsRef<str>) -> Result<String> {
        self.exec(path, |git| {
            let commit = commit.as_ref();
            let output = Command::new(git)
                .arg("rev-parse")
                .arg(commit)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;

            let stdout = String::from_utf8(output.stdout)?;

            if !output.status.success() {
                let stderr = String::from_utf8(output.stderr)?;
                println!("{stderr}");
                return Err(Error::Command(format!("Failed to get hash of {commit}")));
            }

            if let Some(ret) = stdout.split('\n').next() {
                Ok(ret.into())
            } else {
                Err(Error::Command(format!("Failed to get hash of {commit}")))
            }
        })
    }

    fn change_currentdir(to: impl AsRef<Path>) -> Result<PathBuf> {
        let dir = current_dir()?;
        env::set_current_dir(to)?;
        Ok(dir)
    }
}
