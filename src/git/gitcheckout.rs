use super::Git;
use super::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct GitCheckout {
    inner: Git,
    commit: String,
    root_dir: PathBuf,
}

impl GitCheckout {
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

    pub fn checkout(&self, path: &str) -> Result<PathBuf> {
        self.inner.exec(&self.root_dir, |git| {
            let args = vec!["checkout", &self.commit, path];
            let output = Command::new(git)
                .args(args)
                .stderr(Stdio::piped())
                .output()?;
            let stderr = String::from_utf8(output.stderr)?;

            if !output.status.success() {
                println!("{stderr}");
                return Err(Error::Command("Failed to checkout".into()));
            }

            Ok(self.root_dir.join(path))
        })
    }
}
