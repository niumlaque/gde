use super::Git;
use super::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct GitWorktree {
    inner: Git,
    root_dir: PathBuf,
}

impl GitWorktree {
    pub fn new(git: impl AsRef<Path>, target_dir: impl AsRef<Path>) -> Result<Self> {
        let git = Git::from_path(git)?;
        let root_dir = git.get_rootdir(target_dir.as_ref())?;
        Ok(Self {
            inner: git,
            root_dir,
        })
    }

    pub fn add_detached(
        &self,
        worktree_dir: impl AsRef<Path>,
        commit: impl AsRef<str>,
    ) -> Result<()> {
        let worktree_dir = worktree_dir.as_ref();
        let commit = commit.as_ref();
        let output = Command::new(&self.inner.path)
            .arg("worktree")
            .arg("add")
            .arg("--detach")
            .arg(worktree_dir)
            .arg(commit)
            .current_dir(&self.root_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)?;
            return Err(Error::Command(stderr));
        }

        Ok(())
    }

    pub fn remove_force(&self, worktree_dir: impl AsRef<Path>) -> Result<()> {
        let worktree_dir = worktree_dir.as_ref();
        let output = Command::new(&self.inner.path)
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(worktree_dir)
            .current_dir(&self.root_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)?;
            return Err(Error::Command(stderr));
        }

        Ok(())
    }
}
