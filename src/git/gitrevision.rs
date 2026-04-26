use super::Git;
use super::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct GitRevision {
    inner: Git,
    root_dir: PathBuf,
}

impl GitRevision {
    pub fn new(git: impl AsRef<Path>, target_dir: impl AsRef<Path>) -> Result<Self> {
        let git = Git::from_path(git)?;
        let root_dir = git.get_rootdir(target_dir.as_ref())?;
        Ok(Self {
            inner: git,
            root_dir,
        })
    }

    pub fn commit_timestamp(&self, commit: &str) -> Result<i64> {
        self.read_single_value(
            &["show", "-s", "--format=%ct", commit],
            &format!("Failed to get commit timestamp for {commit}"),
        )?
        .parse::<i64>()
        .map_err(|_| Error::Command(format!("Failed to parse commit timestamp for {commit}")))
    }

    pub fn short_hash(&self, commit: &str) -> Result<String> {
        self.read_single_value(
            &["rev-parse", "--short", commit],
            &format!("Failed to get short hash for {commit}"),
        )
    }

    fn read_single_value(&self, args: &[&str], message: &str) -> Result<String> {
        self.inner.exec(&self.root_dir, |git| {
            let output = Command::new(git)
                .args(args)
                .current_dir(&self.root_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8(output.stderr)?;
                return Err(Error::Command(format!("{message} ({stderr})")));
            }

            let stdout = String::from_utf8(output.stdout)?;
            stdout
                .lines()
                .next()
                .map(|line| line.to_string())
                .ok_or_else(|| Error::Command(message.to_string()))
        })
    }
}
