use anyhow::Result;
use clap::Parser;
use gde::FilesCopy;
use std::env;
use std::io::stdout;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
struct Cli {
    /// Path to Git executable used when Git is not in the system PATH
    #[arg(long, value_name = "GIT EXECUTABLE")]
    git: Option<PathBuf>,

    /// Get all differences from this commit
    #[arg(long, value_name = "FROM COMMIT")]
    from: String,

    /// Get all differences up to this commit
    #[arg(long, value_name = "TO COMMIT")]
    to: String,

    /// Destination for output files
    #[arg(short, long, value_name = "OUTPUT DIR")]
    output: Option<PathBuf>,

    /// Path to the git-managed directory for diff
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
    println!("Git version: {}", git.version());

    let target_dir = if let Some(dir) = cli.target {
        absolute_path(dir)?
    } else {
        env::current_dir()?
    };
    println!("Target directory: {}", target_dir.display());
    println!(
        "Root directory: {}",
        git.get_rootdir(&target_dir)?.display()
    );
    let mut output_dir = if let Some(dir) = cli.output {
        absolute_path(dir)?
    } else {
        env::current_dir()?
    };
    output_dir.push(format!("gde-{}", uuid::Uuid::new_v4()));
    println!("Output directory: {}", output_dir.display());

    let current_commit = git.get_hash(&target_dir, "HEAD")?;
    println!("Current commit: {}", current_commit);

    let f = FilesCopy::new(
        git_path,
        cli.from,
        cli.to,
        target_dir,
        output_dir,
        current_commit,
    );

    let out = stdout();
    let mut out = BufWriter::new(out.lock());
    f.copy(&mut out)?;

    Ok(())
}
