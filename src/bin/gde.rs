use anyhow::Result;
use clap::{Parser, Subcommand};
use gde::{AutoCopy, FilesCopy};
use std::env;
use std::io::stdout;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to Git executable used when Git is not in the system PATH
    #[arg(long, value_name = "GIT EXECUTABLE")]
    git: Option<PathBuf>,

    /// Get all differences from this commit
    #[arg(long, value_name = "FROM COMMIT")]
    from: Option<String>,

    /// Get all differences up to this commit
    #[arg(long, value_name = "TO COMMIT")]
    to: Option<String>,

    /// Destination for output files
    #[arg(short, long, value_name = "OUTPUT DIR")]
    output: Option<PathBuf>,

    /// Path to the git-managed directory for diff
    #[arg(value_name = "TARGET REPO DIR")]
    target: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Auto(AutoArgs),
}

#[derive(Debug, Parser)]
struct AutoArgs {
    /// Get all differences from this commit
    #[arg(long, value_name = "FROM COMMIT")]
    from: String,

    /// Branches with head timestamps within this number of days are processed
    #[arg(long, default_value_t = 30, value_name = "DAYS")]
    days: u64,

    /// Destination for output files
    #[arg(short, long, value_name = "OUTPUT DIR")]
    output: PathBuf,

    /// Branch name to exclude from processing
    #[arg(long, value_name = "BRANCH NAME")]
    exclude: Vec<String>,

    /// Append the branch head short hash to the output directory name
    #[arg(long)]
    output_with_short_hash: bool,

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

    let out = stdout();
    let mut out = BufWriter::new(out.lock());
    match cli.command {
        None => {
            let from = cli
                .from
                .ok_or_else(|| anyhow::anyhow!("--from is required in single mode"))?;
            let to = cli
                .to
                .ok_or_else(|| anyhow::anyhow!("--to is required in single mode"))?;
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
            let output_dir = if let Some(dir) = cli.output {
                absolute_path(dir)?
            } else {
                env::current_dir()?.join(format!("gde-{}", uuid::Uuid::new_v4()))
            };
            println!("Output directory: {}", output_dir.display());

            let current_commit = git.get_hash(&target_dir, "HEAD")?;
            println!("Current commit: {}", current_commit);

            let f = FilesCopy::new(git_path, from, to, target_dir, output_dir, current_commit);
            f.copy(&mut out)?;
        }
        Some(Commands::Auto(auto)) => {
            let target_dir = if let Some(dir) = auto.target {
                absolute_path(dir)?
            } else {
                env::current_dir()?
            };
            println!("Target directory: {}", target_dir.display());
            println!(
                "Root directory: {}",
                git.get_rootdir(&target_dir)?.display()
            );
            let output_dir = absolute_path(auto.output)?;
            println!("Output directory: {}", output_dir.display());

            let auto_copy = AutoCopy::new(
                git_path,
                auto.from,
                auto.days,
                target_dir,
                output_dir,
                auto.exclude,
                auto.output_with_short_hash,
            );
            auto_copy.copy(&mut out)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_supports_existing_single_mode_arguments() {
        let cli = Cli::try_parse_from([
            "gde", "--from", "abc123", "--to", "def456", "-o", "out", ".",
        ])
        .unwrap();

        assert!(cli.command.is_none());
        assert_eq!(cli.from, Some("abc123".to_string()));
        assert_eq!(cli.to, Some("def456".to_string()));
        assert_eq!(cli.output, Some(PathBuf::from("out")));
        assert_eq!(cli.target, Some(PathBuf::from(".")));
    }

    #[test]
    fn cli_supports_auto_subcommand_arguments() {
        let cli = Cli::try_parse_from([
            "gde",
            "auto",
            "--from",
            "abc123",
            "--days",
            "15",
            "-o",
            "out",
            "--exclude",
            "main",
            "--exclude",
            "master",
            "--output-with-short-hash",
            ".",
        ])
        .unwrap();

        match cli.command {
            Some(Commands::Auto(auto)) => {
                assert_eq!(auto.from, "abc123");
                assert_eq!(auto.days, 15);
                assert_eq!(auto.output, PathBuf::from("out"));
                assert_eq!(auto.exclude, vec!["main".to_string(), "master".to_string()]);
                assert!(auto.output_with_short_hash);
                assert_eq!(auto.target, Some(PathBuf::from(".")));
            }
            None => panic!("auto command was not parsed"),
        }
    }

    #[test]
    fn cli_command_structure_is_valid() {
        Cli::command().debug_assert();
    }
}
