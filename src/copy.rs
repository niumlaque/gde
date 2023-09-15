use crate::git::{GitCheckout, GitDiff, GitLsTree, GitReset};
use crate::Defer;
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

        let gitdiff = GitDiff::new(
            &self.git_path,
            &self.from_commit,
            Some(&self.to_commit),
            &self.target_dir,
        )?;
        let files = gitdiff.name_only()?;
        if files.is_empty() {
            writeln!(
                w,
                "There are no files with differences between {} and {}",
                self.from_commit, self.to_commit
            )?;
            return Ok(());
        }

        writeln!(
            w,
            "Updated files between {} and {}:",
            self.from_commit, self.to_commit
        )?;
        for file in files.iter() {
            writeln!(w, "\t{}", file)?;
        }

        // check output directory
        fs::create_dir_all(&self.output_dir)?;

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
        let gr = GitReset::new(self.git_path, self.commit, self.target_dir)?;
        let _defer = Defer::new(|| gr.hard().unwrap());

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

                let _ = gc_origin.checkout(file);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use outdir_tempdir::TempDir;
    use std::env;
    use std::fs::File;
    use tar::Archive;

    fn get_test_file() -> PathBuf {
        env::current_dir().unwrap().join("tests").join("gde.tar.gz")
    }

    struct NullWriter;
    impl Write for NullWriter {
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(buf.len())
        }
    }

    #[test]
    fn test_copy() {
        let dir = TempDir::new().autorm();
        let tempdir = dir.path();
        let f = File::open(get_test_file()).unwrap();
        let tar = GzDecoder::new(f);
        let mut archive = Archive::new(tar);
        archive.unpack(&tempdir).unwrap();
        let target_dir = tempdir.join("gde");
        let output_dir = tempdir.join("out");

        let f = FilesCopy::new(
            "git",
            "39fcdfc",
            "4116e23",
            &target_dir,
            &output_dir,
            "HEAD",
        );

        let mut null = NullWriter;
        f.copy(&mut null).unwrap();

        let from_dir = output_dir.join("from");
        let to_dir = output_dir.join("to");

        let from_files = glob::glob(&format!("{}", from_dir.join("**").join("*").display()))
            .unwrap()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        let from_dirs = from_files.iter().filter(|x| x.is_dir()).collect::<Vec<_>>();
        let from_files = from_files
            .iter()
            .filter(|x| x.is_file())
            .collect::<Vec<_>>();
        assert_eq!(3, from_dirs.len());
        assert_eq!(3, from_files.len());

        let to_files = glob::glob(&format!("{}", to_dir.join("**").join("*").display()))
            .unwrap()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        let to_dirs = to_files.iter().filter(|x| x.is_dir()).collect::<Vec<_>>();
        let to_files = to_files.iter().filter(|x| x.is_file()).collect::<Vec<_>>();
        assert_eq!(3, to_dirs.len());
        assert_eq!(3, to_files.len());

        assert!(from_dir.join("README.md").exists());
        assert!(from_dir.join("src").join("bin").join("gde.rs").exists());
        assert!(from_dir.join("src").join("git").join("mod.rs").exists());
        assert!(to_dir.join("README.md").exists());
        assert!(to_dir.join("src").join("bin").join("gde.rs").exists());
        assert!(to_dir.join("src").join("git").join("mod.rs").exists());
    }
}
