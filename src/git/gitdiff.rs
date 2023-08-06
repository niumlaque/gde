use super::Git;
use super::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct GitDiff {
    inner: Git,
    from: String,
    to: String,
    root_dir: PathBuf,
}

impl GitDiff {
    pub fn new(
        git: impl AsRef<Path>,
        from: impl Into<String>,
        to: impl Into<String>,
        target_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        let git = Git::from_path(git)?;
        let root_dir = git.get_rootdir(target_dir.as_ref())?;
        Ok(Self {
            inner: git,
            from: from.into(),
            to: to.into(),
            root_dir,
        })
    }

    pub fn name_only(&self) -> Result<Vec<String>> {
        self.inner.exec(&self.root_dir, |git| {
            let output = Command::new(git)
                .arg("diff")
                .arg("--name-only")
                .arg(&self.from)
                .arg(&self.to)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;
            let stdout = String::from_utf8(output.stdout)?;
            let stderr = String::from_utf8(output.stderr)?;

            if !stderr.is_empty() {
                return Err(Error::Command("Failed to get root directory".into()));
            }

            Ok(stdout
                .split('\n')
                .filter(|x| !x.is_empty())
                .map(|x| x.into())
                .collect::<Vec<_>>())
        })
    }
}
