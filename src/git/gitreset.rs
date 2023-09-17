use super::Git;
use super::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct GitReset {
    inner: Git,
    commit: String,
    root_dir: PathBuf,
}

impl GitReset {
    pub fn new(
        git: impl AsRef<Path>,
        commit: impl Into<String>,
        target_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        let git = Git::from_path(git)?;
        let root_dir = git.get_rootdir(target_dir.as_ref())?;
        Ok(Self {
            inner: git,
            commit: commit.into(),
            root_dir,
        })
    }

    pub fn hard(&self) -> Result<()> {
        self.inner.exec(&self.root_dir, |git| {
            let args = vec!["reset", "--hard", &self.commit];
            let output = Command::new(git)
                .args(args)
                .stderr(Stdio::piped())
                .output()?;
            let stderr = String::from_utf8(output.stderr)?;

            if !output.status.success() {
                return Err(Error::Command(stderr));
            }

            Ok(())
        })
    }
}
