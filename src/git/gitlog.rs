use super::Git;
use super::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct GitLog {
    inner: Git,
    all: bool,
    root_dir: PathBuf,
}

impl GitLog {
    pub fn new(git: impl AsRef<Path>, all: bool, target_dir: impl AsRef<Path>) -> Result<Self> {
        let git = Git::from_path(git)?;
        let root_dir = git.get_rootdir(target_dir.as_ref())?;
        Ok(Self {
            inner: git,
            all,
            root_dir,
        })
    }

    pub fn tree(&self) -> Result<Vec<String>> {
        self.inner.exec(&self.root_dir, |git| {
            let mut args = vec!["log", "--graph"];
            if self.all {
                args.push("--all");
            }
            args.push("--pretty=format:%h -%d %s (%ci) <%an>");
            args.push("--abbrev-commit");
            args.push("--date=relative");
            let output = Command::new(git)
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;
            let stdout = String::from_utf8(output.stdout)?;
            let stderr = String::from_utf8(output.stderr)?;

            if !output.status.success() {
                return Err(Error::Command(format!("Failed to get logs ({stderr})")));
            }

            Ok(stdout
                .split('\n')
                .map(|x| x.to_string())
                .collect::<Vec<_>>())
        })
    }
}
