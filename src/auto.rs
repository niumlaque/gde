use crate::git::{Git, GitLocalBranch, GitLocalBranches, GitMergeBase, GitRevision};
use crate::FilesCopy;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;

pub struct AutoCopy {
    git_path: PathBuf,
    from_commit: String,
    days: u64,
    target_dir: PathBuf,
    output_dir: PathBuf,
    excludes: HashSet<String>,
    output_with_short_hash: bool,
}

impl AutoCopy {
    pub fn new(
        git_path: impl Into<PathBuf>,
        from_commit: impl Into<String>,
        days: u64,
        target_dir: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
        excludes: impl IntoIterator<Item = String>,
        output_with_short_hash: bool,
    ) -> Self {
        Self {
            git_path: git_path.into(),
            from_commit: from_commit.into(),
            days,
            target_dir: target_dir.into(),
            output_dir: output_dir.into(),
            excludes: excludes.into_iter().collect(),
            output_with_short_hash,
        }
    }

    pub fn copy<W: Write>(&self, w: &mut W) -> Result<()> {
        let git = Git::from_path(&self.git_path)?;
        let root_dir = git.get_rootdir(&self.target_dir)?;
        let from_hash = git.get_hash(&root_dir, &self.from_commit)?;
        let current_commit = git.get_hash(&root_dir, "HEAD")?;
        let revision = GitRevision::new(&self.git_path, &root_dir)?;
        let branches = GitLocalBranches::new(&self.git_path, &root_dir)?;
        let merge_base = GitMergeBase::new(&self.git_path, &root_dir)?;
        let from_timestamp = revision.commit_timestamp(&from_hash)?;
        let max_delta = self.days.saturating_mul(86_400);

        writeln!(w, "Base commit: {from_hash}")?;
        writeln!(w, "Days: {}", self.days)?;
        writeln!(w, "Output directory: {}", self.output_dir.display())?;

        let mut selected = Vec::new();
        for branch in branches.list()? {
            match self.selection_reason(
                &branch,
                &from_hash,
                from_timestamp,
                max_delta,
                &merge_base,
            )? {
                Some(reason) => {
                    writeln!(w, "Skipped branch: {} ({reason})", branch.name)?;
                }
                None => selected.push(branch),
            }
        }

        let output_dirs = self.resolve_output_dirs(&selected, &revision)?;
        writeln!(w, "Selected branch count: {}", selected.len())?;

        for branch in selected {
            let branch_output_dir = output_dirs
                .get(&branch.name)
                .cloned()
                .unwrap_or_else(|| sanitize_branch_name(&branch.name));
            let output_dir = self.output_dir.join(&branch_output_dir);
            writeln!(
                w,
                "Processed branch: {} -> {}",
                branch.name,
                output_dir.display()
            )?;

            let copy = FilesCopy::new(
                self.git_path.clone(),
                from_hash.clone(),
                branch.head_hash,
                root_dir.clone(),
                output_dir,
                current_commit.clone(),
            );
            copy.copy(w)?;
        }

        Ok(())
    }

    fn selection_reason(
        &self,
        branch: &GitLocalBranch,
        from_hash: &str,
        from_timestamp: i64,
        max_delta: u64,
        merge_base: &GitMergeBase,
    ) -> Result<Option<String>> {
        if self.excludes.contains(&branch.name) {
            return Ok(Some("excluded".to_string()));
        }

        if branch.head_hash == from_hash {
            return Ok(Some("head matches base commit".to_string()));
        }

        if !merge_base.is_ancestor(from_hash, &branch.head_hash)? {
            return Ok(Some("base commit is not an ancestor".to_string()));
        }

        if branch.committer_timestamp < from_timestamp {
            return Ok(Some("head timestamp is older than base commit".to_string()));
        }

        let delta = branch.committer_timestamp - from_timestamp;
        if delta as u64 > max_delta {
            return Ok(Some("outside --days range".to_string()));
        }

        Ok(None)
    }

    fn resolve_output_dirs(
        &self,
        branches: &[GitLocalBranch],
        revision: &GitRevision,
    ) -> Result<HashMap<String, String>> {
        self.resolve_output_dirs_with(branches, |head_hash| Ok(revision.short_hash(head_hash)?))
    }

    fn resolve_output_dirs_with<F>(
        &self,
        branches: &[GitLocalBranch],
        mut short_hash: F,
    ) -> Result<HashMap<String, String>>
    where
        F: FnMut(&str) -> Result<String>,
    {
        let mut counts = HashMap::new();
        let mut sanitized = HashMap::new();
        for branch in branches {
            let name = sanitize_branch_name(&branch.name);
            *counts.entry(name.clone()).or_insert(0usize) += 1;
            sanitized.insert(branch.name.clone(), name);
        }

        let mut used = HashSet::new();
        let mut resolved = HashMap::new();
        for branch in branches {
            let base = sanitized
                .get(&branch.name)
                .cloned()
                .unwrap_or_else(|| sanitize_branch_name(&branch.name));
            let is_conflicted = counts.get(&base).copied().unwrap_or_default() > 1;
            let mut output_dir = base.clone();
            if self.output_with_short_hash || (is_conflicted && used.contains(&base)) {
                let suffix = short_hash(&branch.head_hash)?;
                output_dir = format!("{base}_{suffix}");
            }

            let output_dir = make_unique_output_dir(output_dir, &mut used);

            resolved.insert(branch.name.clone(), output_dir);
        }

        Ok(resolved)
    }
}

pub fn sanitize_branch_name(name: &str) -> String {
    let mut chars = name
        .chars()
        .map(|c| {
            if is_invalid_branch_output_char(c) {
                '_'
            } else {
                c
            }
        })
        .collect::<Vec<_>>();

    while let Some(c) = chars.first_mut() {
        if *c == ' ' || *c == '.' {
            *c = '_';
        } else {
            break;
        }
    }

    while let Some(c) = chars.last_mut() {
        if *c == ' ' || *c == '.' {
            *c = '_';
        } else {
            break;
        }
    }

    let sanitized = chars.into_iter().collect::<String>();
    if sanitized.is_empty() {
        "branch".to_string()
    } else {
        sanitized
    }
}

fn is_invalid_branch_output_char(c: char) -> bool {
    c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
}

fn make_unique_output_dir(candidate: String, used: &mut HashSet<String>) -> String {
    if used.insert(candidate.clone()) {
        return candidate;
    }

    let mut index = 2;
    loop {
        let suffixed = format!("{candidate}_{index}");
        if used.insert(suffixed.clone()) {
            return suffixed;
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::git_test_lock;
    use outdir_tempdir::TempDir;
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    struct AutoTestRepo {
        _dir: TempDir,
        repo_dir: PathBuf,
        output_dir: PathBuf,
        from_commit: String,
        in_range_branch_hash: String,
    }

    impl AutoTestRepo {
        fn new() -> Self {
            let dir = TempDir::new().autorm();
            let repo_dir = dir.path().join("repo");
            let output_dir = dir.path().join("out");
            fs::create_dir_all(&repo_dir).unwrap();

            run_git(&repo_dir, &["init"]);
            run_git(&repo_dir, &["branch", "-m", "main"]);
            run_git(&repo_dir, &["config", "user.name", "gde tests"]);
            run_git(
                &repo_dir,
                &["config", "user.email", "gde-tests@example.com"],
            );
            write_bytes(repo_dir.join("shared.txt"), b"base\n");
            let from_commit = commit_all_at(&repo_dir, "base", "2024-01-01T00:00:00 +0000");
            run_git(&repo_dir, &["branch", "base-branch", &from_commit]);

            run_git(
                &repo_dir,
                &["checkout", "-b", "feature/in-range", &from_commit],
            );
            write_bytes(repo_dir.join("shared.txt"), b"in range\n");
            let in_range_branch_hash =
                commit_all_at(&repo_dir, "in range", "2024-01-10T00:00:00 +0000");

            run_git(&repo_dir, &["checkout", "main"]);
            run_git(
                &repo_dir,
                &["checkout", "-b", "feature/out-of-range", &from_commit],
            );
            write_bytes(repo_dir.join("shared.txt"), b"out of range\n");
            commit_all_at(&repo_dir, "out of range", "2024-02-20T00:00:00 +0000");

            run_git(&repo_dir, &["checkout", "main"]);
            run_git(
                &repo_dir,
                &["checkout", "-b", "feature/excluded", &from_commit],
            );
            write_bytes(repo_dir.join("excluded.txt"), b"excluded\n");
            commit_all_at(&repo_dir, "excluded", "2024-01-11T00:00:00 +0000");

            run_git(&repo_dir, &["checkout", "main"]);
            run_git(
                &repo_dir,
                &["checkout", "-b", "feature/short-hash", &from_commit],
            );
            write_bytes(repo_dir.join("short.txt"), b"short hash\n");
            commit_all_at(&repo_dir, "short hash", "2024-01-12T00:00:00 +0000");

            run_git(&repo_dir, &["checkout", "main"]);
            run_git(&repo_dir, &["checkout", "--orphan", "unrelated"]);
            remove_all_files(&repo_dir);
            write_bytes(repo_dir.join("unrelated.txt"), b"unrelated\n");
            commit_all_at(&repo_dir, "unrelated", "2024-01-13T00:00:00 +0000");

            run_git(&repo_dir, &["checkout", "main"]);
            run_git(&repo_dir, &["checkout", "-b", "remote-only", &from_commit]);
            write_bytes(repo_dir.join("remote.txt"), b"remote\n");
            let remote_hash = commit_all_at(&repo_dir, "remote", "2024-01-14T00:00:00 +0000");
            run_git(
                &repo_dir,
                &[
                    "update-ref",
                    "refs/remotes/origin/remote-only",
                    &remote_hash,
                ],
            );
            run_git(&repo_dir, &["checkout", "main"]);
            run_git(&repo_dir, &["branch", "-D", "remote-only"]);

            Self {
                _dir: dir,
                repo_dir,
                output_dir,
                from_commit,
                in_range_branch_hash,
            }
        }

        fn auto_copy(
            &self,
            days: u64,
            excludes: &[&str],
            output_with_short_hash: bool,
        ) -> AutoCopy {
            AutoCopy::new(
                "git",
                self.from_commit.clone(),
                days,
                self.repo_dir.clone(),
                self.output_dir.clone(),
                excludes.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
                output_with_short_hash,
            )
        }
    }

    fn run_git(repo_dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_dir)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "git {:?} failed: stdout={}, stderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8(output.stdout).unwrap()
    }

    fn commit_all_at(repo_dir: &Path, message: &str, date: &str) -> String {
        let output = Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_dir)
            .output()
            .unwrap();
        assert!(output.status.success());

        let output = Command::new("git")
            .args(["commit", "-m", message])
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .current_dir(repo_dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git commit failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        run_git(repo_dir, &["rev-parse", "HEAD"]).trim().to_string()
    }

    fn write_bytes(path: impl AsRef<Path>, bytes: &[u8]) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    fn remove_all_files(repo_dir: &Path) {
        for entry in fs::read_dir(repo_dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() == ".git" {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(path).unwrap();
            } else {
                fs::remove_file(path).unwrap();
            }
        }
    }

    #[test]
    fn sanitize_branch_name_replaces_invalid_characters() {
        assert_eq!("feature_name", sanitize_branch_name("feature/name"));
        assert_eq!("_branch_", sanitize_branch_name(".branch."));
        assert_eq!("___", sanitize_branch_name(":?*"));
        assert_eq!("branch", sanitize_branch_name(""));
    }

    #[test]
    fn auto_selects_only_local_branches_in_range() {
        let _lock = git_test_lock();
        let repo = AutoTestRepo::new();
        let mut out = Vec::new();

        repo.auto_copy(30, &[], false).copy(&mut out).unwrap();

        assert!(repo
            .output_dir
            .join("feature_in-range")
            .join("from")
            .exists());
        assert!(repo.output_dir.join("feature_in-range").join("to").exists());
        assert!(!repo.output_dir.join("feature_out-of-range").exists());
        assert!(repo.output_dir.join("feature_excluded").join("to").exists());
        assert!(!repo.output_dir.join("base-branch").exists());
        assert!(!repo.output_dir.join("unrelated").exists());
        assert!(!repo.output_dir.join("remote-only").exists());

        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("Selected branch count: 3"));
        assert!(output.contains("Skipped branch: base-branch (head matches base commit)"));
        assert!(output.contains("Skipped branch: feature/out-of-range (outside --days range)"));
        assert!(output.contains("Skipped branch: unrelated (base commit is not an ancestor)"));
    }

    #[test]
    fn auto_honors_exclude_option() {
        let _lock = git_test_lock();
        let repo = AutoTestRepo::new();
        let mut out = Vec::new();

        repo.auto_copy(30, &["main", "feature/excluded"], false)
            .copy(&mut out)
            .unwrap();

        assert!(!repo.output_dir.join("feature_excluded").exists());
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("Skipped branch: main (excluded)"));
        assert!(output.contains("Skipped branch: feature/excluded (excluded)"));
    }

    #[test]
    fn auto_appends_short_hash_only_when_requested() {
        let _lock = git_test_lock();
        let repo = AutoTestRepo::new();
        let short_hash = run_git(
            &repo.repo_dir,
            &["rev-parse", "--short", &repo.in_range_branch_hash],
        )
        .trim()
        .to_string();
        let mut out = Vec::new();

        repo.auto_copy(30, &[], true).copy(&mut out).unwrap();

        assert!(repo
            .output_dir
            .join(format!("feature_in-range_{short_hash}"))
            .join("from")
            .exists());
    }

    #[test]
    fn auto_avoids_overwriting_when_sanitized_names_collide() {
        let auto = AutoCopy::new("git", "HEAD", 30, ".", "out", Vec::<String>::new(), false);
        let branches = vec![
            GitLocalBranch {
                name: "feature/foo".to_string(),
                head_hash: "1111111111111111111111111111111111111111".to_string(),
                committer_timestamp: 1,
            },
            GitLocalBranch {
                name: "feature:foo".to_string(),
                head_hash: "1111111111111111111111111111111111111111".to_string(),
                committer_timestamp: 2,
            },
            GitLocalBranch {
                name: "feature*foo".to_string(),
                head_hash: "1111111111111111111111111111111111111111".to_string(),
                committer_timestamp: 3,
            },
        ];

        let resolved = auto
            .resolve_output_dirs_with(&branches, |head_hash| Ok(head_hash[..7].to_string()))
            .unwrap();
        assert_eq!(resolved.get("feature/foo").unwrap(), "feature_foo");
        assert_eq!(resolved.get("feature:foo").unwrap(), "feature_foo_1111111");
        assert_eq!(
            resolved.get("feature*foo").unwrap(),
            "feature_foo_1111111_2"
        );
    }

    #[test]
    fn auto_avoids_overwriting_when_short_hash_output_names_collide() {
        let auto = AutoCopy::new("git", "HEAD", 30, ".", "out", Vec::<String>::new(), true);
        let branches = vec![
            GitLocalBranch {
                name: "feature/foo".to_string(),
                head_hash: "1111111111111111111111111111111111111111".to_string(),
                committer_timestamp: 1,
            },
            GitLocalBranch {
                name: "feature:foo".to_string(),
                head_hash: "1111111111111111111111111111111111111111".to_string(),
                committer_timestamp: 2,
            },
            GitLocalBranch {
                name: "feature*foo".to_string(),
                head_hash: "1111111111111111111111111111111111111111".to_string(),
                committer_timestamp: 3,
            },
        ];

        let resolved = auto
            .resolve_output_dirs_with(&branches, |head_hash| Ok(head_hash[..7].to_string()))
            .unwrap();
        assert_eq!(resolved.get("feature/foo").unwrap(), "feature_foo_1111111");
        assert_eq!(
            resolved.get("feature:foo").unwrap(),
            "feature_foo_1111111_2"
        );
        assert_eq!(
            resolved.get("feature*foo").unwrap(),
            "feature_foo_1111111_3"
        );
    }

    #[test]
    fn auto_preserves_working_tree_staged_and_untracked_files() {
        let _lock = git_test_lock();
        let repo = AutoTestRepo::new();
        write_bytes(repo.repo_dir.join("shared.txt"), b"dirty\n");
        run_git(&repo.repo_dir, &["add", "shared.txt"]);
        write_bytes(repo.repo_dir.join("local-only.txt"), b"local\n");
        let head_before = run_git(&repo.repo_dir, &["rev-parse", "HEAD"])
            .trim()
            .to_string();
        let mut out = Vec::new();

        repo.auto_copy(30, &[], false).copy(&mut out).unwrap();

        assert_eq!(
            head_before,
            run_git(&repo.repo_dir, &["rev-parse", "HEAD"])
                .trim()
                .to_string()
        );
        assert_eq!("", run_git(&repo.repo_dir, &["diff", "--name-only"]).trim());
        assert_eq!(
            "shared.txt",
            run_git(&repo.repo_dir, &["diff", "--staged", "--name-only"]).trim()
        );
        assert!(repo.repo_dir.join("local-only.txt").exists());
    }
}
