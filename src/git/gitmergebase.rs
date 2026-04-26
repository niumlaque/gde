use super::Git;
use super::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct GitMergeBase {
    inner: Git,
    root_dir: PathBuf,
}

impl GitMergeBase {
    pub fn new(git: impl AsRef<Path>, target_dir: impl AsRef<Path>) -> Result<Self> {
        let git = Git::from_path(git)?;
        let root_dir = git.get_rootdir(target_dir.as_ref())?;
        Ok(Self {
            inner: git,
            root_dir,
        })
    }

    pub fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool> {
        self.inner.exec(&self.root_dir, |git| {
            let output = Command::new(git)
                .args(["merge-base", "--is-ancestor", ancestor, descendant])
                .current_dir(&self.root_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;

            match output.status.code() {
                Some(0) => Ok(true),
                Some(1) => Ok(false),
                _ => {
                    let stderr = String::from_utf8(output.stderr)?;
                    Err(Error::Command(format!(
                        "Failed to check merge-base ancestry ({stderr})"
                    )))
                }
            }
        })
    }
}
