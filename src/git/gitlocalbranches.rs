use super::Git;
use super::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLocalBranch {
    pub name: String,
    pub head_hash: String,
    pub committer_timestamp: i64,
}

pub struct GitLocalBranches {
    inner: Git,
    root_dir: PathBuf,
}

impl GitLocalBranches {
    pub fn new(git: impl AsRef<Path>, target_dir: impl AsRef<Path>) -> Result<Self> {
        let git = Git::from_path(git)?;
        let root_dir = git.get_rootdir(target_dir.as_ref())?;
        Ok(Self {
            inner: git,
            root_dir,
        })
    }

    pub fn list(&self) -> Result<Vec<GitLocalBranch>> {
        self.inner.exec(&self.root_dir, |git| {
            let output = Command::new(git)
                .args([
                    "for-each-ref",
                    "refs/heads",
                    "--format=%(refname:short)\t%(objectname)\t%(committerdate:unix)",
                ])
                .current_dir(&self.root_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8(output.stderr)?;
                return Err(Error::Command(format!(
                    "Failed to list local branches ({stderr})"
                )));
            }

            let stdout = String::from_utf8(output.stdout)?;
            let mut branches = Vec::new();
            for line in stdout.lines().filter(|line| !line.is_empty()) {
                let fields = line.split('\t').collect::<Vec<_>>();
                if fields.len() != 3 {
                    return Err(Error::Command("Failed to parse local branches".to_string()));
                }

                let timestamp = fields[2].parse::<i64>().map_err(|_| {
                    Error::Command(format!(
                        "Failed to parse branch timestamp for {}",
                        fields[0]
                    ))
                })?;
                branches.push(GitLocalBranch {
                    name: fields[0].to_string(),
                    head_hash: fields[1].to_string(),
                    committer_timestamp: timestamp,
                });
            }

            Ok(branches)
        })
    }
}
