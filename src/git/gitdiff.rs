use super::Git;
use super::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

enum StagedOption {
    NotStaged,
    Staged,
}

pub struct GitDiff {
    inner: Git,
    from: String,
    to: Option<String>,
    root_dir: PathBuf,
}

impl GitDiff {
    pub fn new(
        git: impl AsRef<Path>,
        from: impl Into<String>,
        to: Option<impl Into<String>>,
        target_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        let git = Git::from_path(git)?;
        let root_dir = git.get_rootdir(target_dir.as_ref())?;
        Ok(Self {
            inner: git,
            from: from.into(),
            to: to.map(Into::into),
            root_dir,
        })
    }

    pub fn name_only(&self) -> Result<Vec<String>> {
        self.inner_name_only(StagedOption::NotStaged)
    }

    pub fn staged_name_only(&self) -> Result<Vec<String>> {
        self.inner_name_only(StagedOption::Staged)
    }

    fn inner_name_only(&self, staged: StagedOption) -> Result<Vec<String>> {
        self.inner.exec(&self.root_dir, |git| {
            let mut args = vec!["diff"];
            if let StagedOption::Staged = staged {
                args.push("--staged");
            }
            args.extend(vec!["--name-only", &self.from]);
            if let Some(to) = self.to.as_ref() {
                args.push(to);
            }
            let output = Command::new(git)
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;
            let stdout = String::from_utf8(output.stdout)?;
            let stderr = String::from_utf8(output.stderr)?;

            if !output.status.success() {
                return Err(Error::Command(format!(
                    "Failed to get differences ({stderr})"
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
