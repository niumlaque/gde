use super::Git;
use super::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct GitLsTree {
    inner: Git,
    commit: String,
    root_dir: PathBuf,
}

impl GitLsTree {
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

    pub fn name_only(&self) -> Result<Vec<String>> {
        self.inner.exec(&self.root_dir, |git| {
            let args = vec!["ls-tree", "-r", "--name-only", &self.commit];
            let output = Command::new(git)
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;
            let stdout = String::from_utf8(output.stdout)?;
            let stderr = String::from_utf8(output.stderr)?;

            if !output.status.success() {
                return Err(Error::Command(format!(
                    "Failed to get tree of files ({stderr})"
                )));
            }

            Ok(stdout
                .split('\n')
                .filter(|x| !x.is_empty())
                .map(|x| x.into())
                .collect::<Vec<_>>())
        })
    }
}
