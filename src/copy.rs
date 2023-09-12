use crate::git::{GitCheckout, GitDiff, GitLsTree};
use anyhow::{bail, Result};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Provides a feature to copy the differential files between two specified commits
pub struct FilesCopy {
    /// The path to the git executable
    git_path: PathBuf,

    /// Get all differences from this commit
    from_commit: String,

    /// Get all differences up to this commit
    to_commit: String,

    /// The path to the directory where the files to be copied are located
    target_dir: PathBuf,

    /// The path to the directory for output
    output_dir: PathBuf,

    /// The current commit in the target directory
    current_commit: String,
}

impl FilesCopy {
    pub fn new(
        git_path: impl Into<PathBuf>,
        from_commit: impl Into<String>,
        to_commit: impl Into<String>,
        target_dir: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
        current_commit: impl Into<String>,
    ) -> Self {
        Self {
            git_path: git_path.into(),
            from_commit: from_commit.into(),
            to_commit: to_commit.into(),
            target_dir: target_dir.into(),
            output_dir: output_dir.into(),
            current_commit: current_commit.into(),
        }
    }

    /// Copies the differential files between the commits specified in the constructor
    pub fn copy<W: Write>(&self, w: &mut W) -> Result<()> {
        // check changes
        let gitdiff = GitDiff::new(&self.git_path, "HEAD", None::<String>, &self.target_dir)?;
        if !gitdiff.name_only()?.is_empty() || !gitdiff.staged_name_only()?.is_empty() {
            bail!("Please commit or discard the changes");
        }

        // check output directory
        fs::create_dir_all(&self.output_dir)?;

        let gitdiff = GitDiff::new(
            &self.git_path,
            &self.from_commit,
            Some(&self.to_commit),
            &self.target_dir,
        )?;
        let files = gitdiff.name_only()?;
        writeln!(
            w,
            "Updated files between {} and {}:",
            self.from_commit, self.to_commit
        )?;
        for file in files.iter() {
            writeln!(w, "\t{}", file)?;
        }

        // Copy files from "From Commit"
        let from_dir = self.output_dir.join("from");
        writeln!(w, "Copiying files from \"{}\"...", self.from_commit)?;
        let from = FilesCopyInner::new(
            &self.git_path,
            &files,
            &self.target_dir,
            &self.from_commit,
            &self.current_commit,
            &from_dir,
        );
        from.copy(w)?;

        // Copy files from "To Commit"
        let to_dir = self.output_dir.join("to");
        writeln!(w, "Copiying files from \"{}\"...", self.to_commit)?;
        let to = FilesCopyInner::new(
            &self.git_path,
            &files,
            &self.target_dir,
            &self.to_commit,
            &self.current_commit,
            &to_dir,
        );
        to.copy(w)?;
        Ok(())
    }
}

struct FilesCopyInner<'a> {
    /// The path to the git executable
    git_path: &'a Path,

    /// The files to copy
    target_files: &'a [String],

    /// The path to the directory where the files to be copied are located
    target_dir: &'a Path,

    /// Copy the files from this commit
    commit: &'a str,

    /// The hash of the current commit in the target directory
    original_commit: &'a str,

    /// The path to the directory for output
    output_dir: &'a Path,
}

impl<'a> FilesCopyInner<'a> {
    fn new(
        git_path: &'a Path,
        target_files: &'a [String],
        target_dir: &'a Path,
        commit: &'a str,
        original_commit: &'a str,
        output_dir: &'a Path,
    ) -> Self {
        Self {
            git_path,
            target_files,
            target_dir,
            commit,
            original_commit,
            output_dir,
        }
    }

    fn copy<W: Write>(&self, w: &mut W) -> Result<()> {
        let gitls = GitLsTree::new(self.git_path, self.commit, self.target_dir)?;
        let set = gitls.name_only()?.into_iter().collect::<HashSet<_>>();
        let gc = GitCheckout::new(self.git_path, self.commit, self.target_dir)?;
        let gc_origin = GitCheckout::new(self.git_path, self.original_commit, self.target_dir)?;

        for file in self.target_files.iter() {
            let mut dir = PathBuf::from(file);
            dir.pop();
            let out_dir = self.output_dir.join(dir);
            fs::create_dir_all(&out_dir)?;
            if set.contains(file) {
                let dest_file = self.output_dir.join(file);
                let source_file = gc.checkout(file)?;
                fs::copy(&source_file, &dest_file)?;
                writeln!(
                    w,
                    "Copied: {} -> {}",
                    source_file.display(),
                    dest_file.display()
                )?;

                if let Err(e) = gc_origin.checkout(file) {
                    bail!("Failed to reset {file} to {}({e})", self.original_commit);
                }
            }
        }

        Ok(())
    }
}
