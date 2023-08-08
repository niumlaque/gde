use anyhow::bail;
use anyhow::Result;
use clap::Parser;
use gde::git::{GitCheckout, GitDiff, GitLsTree};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
struct Cli {
    /// Path to git executable
    #[arg(long, value_name = "GIT EXECUTABLE")]
    git: Option<PathBuf>,

    #[arg(long, value_name = "FROM COMMIT")]
    from: String,

    #[arg(long, value_name = "TO COMMIT")]
    to: String,

    #[arg(short, long, value_name = "OUTPUT DIR")]
    output: Option<PathBuf>,

    #[arg(value_name = "TARGET REPO DIR")]
    target: Option<PathBuf>,
}

fn absolute_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    let ret = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };

    Ok(ret)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let git_path = if let Some(git) = cli.git {
        git.display().to_string()
    } else {
        "git".to_string()
    };

    let git = gde::git::Git::from_path(&git_path)?;
    println!("git version: {}", git.version());

    let target_dir = if let Some(dir) = cli.target {
        absolute_path(dir)?
    } else {
        env::current_dir()?
    };
    println!("target directory: {}", target_dir.display());
    println!(
        "root directory: {}",
        git.get_rootdir(&target_dir)?.display()
    );
    let mut output_dir = if let Some(dir) = cli.output {
        absolute_path(dir)?
    } else {
        env::current_dir()?
    };
    output_dir.push(uuid::Uuid::new_v4().to_string());
    println!("output directory: {}", output_dir.display());

    // check changes
    let gitdiff = GitDiff::new(&git_path, "HEAD", None::<String>, &target_dir)?;
    if !gitdiff.name_only()?.is_empty() || !gitdiff.staged_name_only()?.is_empty() {
        bail!("Please commit or discard the changes");
    }

    let gitdiff = GitDiff::new(&git_path, &cli.from, Some(&cli.to), &target_dir)?;
    let files = gitdiff.name_only()?;
    println!("Updated files:");
    for file in files.iter() {
        println!("\t{}", file);
    }

    // From
    let from_dir = output_dir.join("from");
    println!("Copiying `from` files...");
    let from = FilesCopy::new(&git_path, files.iter(), &target_dir, &cli.from, from_dir);
    from.copy()?;

    // To
    let to_dir = output_dir.join("to");
    println!("Copiying `to` files...");
    let to = FilesCopy::new(&git_path, files.iter(), &target_dir, &cli.to, to_dir);
    to.copy()?;

    println!("done");

    Ok(())
}

struct FilesCopy {
    git_path: PathBuf,
    target_files: Vec<String>,
    target_dir: PathBuf,
    commit: String,
    output_dir: PathBuf,
}

impl FilesCopy {
    pub fn new<I: std::iter::Iterator<Item = impl AsRef<str>>>(
        git_path: impl AsRef<Path>,
        target_files: I,
        target_dir: impl AsRef<Path>,
        commit: impl Into<String>,
        output_dir: impl AsRef<Path>,
    ) -> Self {
        let target_files = target_files
            .map(|x| x.as_ref().to_string())
            .collect::<Vec<_>>();
        Self {
            git_path: git_path.as_ref().to_path_buf(),
            target_files,
            target_dir: target_dir.as_ref().to_path_buf(),
            commit: commit.into(),
            output_dir: output_dir.as_ref().to_path_buf(),
        }
    }

    pub fn copy(&self) -> Result<()> {
        let gitls = GitLsTree::new(&self.git_path, &self.commit, &self.target_dir)?;
        let set = gitls.name_only()?.into_iter().collect::<HashSet<_>>();
        let gc = GitCheckout::new(&self.git_path, &self.commit, &self.target_dir)?;

        for file in self.target_files.iter() {
            let mut dir = PathBuf::from(file);
            dir.pop();
            let out_dir = self.output_dir.join(dir);
            fs::create_dir_all(&out_dir)?;
            if set.contains(file) {
                let dest_file = self.output_dir.join(file);
                let source_file = gc.checkout(file)?;
                fs::copy(&source_file, &dest_file)?;
                println!(
                    "Copied: {} -> {}",
                    source_file.display(),
                    dest_file.display()
                );
            }
        }

        Ok(())
    }
}
