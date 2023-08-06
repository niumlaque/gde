use anyhow::Result;
use clap::Parser;
use gde::git::GitDiff;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Cli {
    /// Path to git executable
    #[arg(long, value_name = "GIT EXECUTABLE")]
    git: Option<PathBuf>,

    #[arg(long, value_name = "FROM COMMIT")]
    from: String,

    #[arg(long, value_name = "TO COMMIT")]
    to: String,

    #[arg(value_name = "TARGET REPO DIR")]
    target_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let git_path = if let Some(git) = cli.git {
        git.display().to_string()
    } else {
        "git".to_string()
    };

    let target_dir = if let Some(dir) = cli.target_dir {
        dir
    } else {
        env::current_dir()?
    };

    let git = gde::git::Git::from_path(&git_path)?;
    println!("git version: {}", git.version());
    println!(
        "root directory: {}",
        git.get_rootdir(&target_dir)?.display()
    );

    let gitdiff = GitDiff::new(&git_path, cli.from, cli.to, &target_dir)?;
    let files = gitdiff.name_only()?;
    println!("{:?}", files);

    Ok(())
}
