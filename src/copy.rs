use crate::git::{GitDiff, GitWorktree};
use anyhow::Result;
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
        // Kept for API compatibility with the previous checkout/reset implementation.
        let _ = &self.current_commit;

        let worktree_base_dir = std::env::temp_dir().join("gde-worktrees");
        let worktree_session_dir = worktree_base_dir.join(uuid::Uuid::new_v4().to_string());
        let from_worktree_dir = worktree_session_dir.join("from");
        let to_worktree_dir = worktree_session_dir.join("to");
        let git_worktree = GitWorktree::new(&self.git_path, &self.target_dir)?;
        git_worktree.add_detached(&from_worktree_dir, &self.from_commit)?;
        let _from_guard = WorktreeGuard::new(
            &git_worktree,
            &from_worktree_dir,
            &worktree_session_dir,
            &worktree_base_dir,
        );
        git_worktree.add_detached(&to_worktree_dir, &self.to_commit)?;
        let _to_guard = WorktreeGuard::new(
            &git_worktree,
            &to_worktree_dir,
            &worktree_session_dir,
            &worktree_base_dir,
        );

        // Copy files from "From Commit"
        let from_dir = self.output_dir.join("from");
        writeln!(w, "Copiying files from \"{}\"...", self.from_commit)?;
        let from = FilesCopyInner::new(&files, &from_worktree_dir, &from_dir);
        from.copy(w)?;

        // Copy files from "To Commit"
        let to_dir = self.output_dir.join("to");
        writeln!(w, "Copiying files from \"{}\"...", self.to_commit)?;
        let to = FilesCopyInner::new(&files, &to_worktree_dir, &to_dir);
        to.copy(w)?;
        Ok(())
    }
}

struct WorktreeGuard<'a> {
    git_worktree: &'a GitWorktree,
    worktree_dir: &'a Path,
    worktree_session_dir: &'a Path,
    worktree_base_dir: &'a Path,
}

impl<'a> WorktreeGuard<'a> {
    fn new(
        git_worktree: &'a GitWorktree,
        worktree_dir: &'a Path,
        worktree_session_dir: &'a Path,
        worktree_base_dir: &'a Path,
    ) -> Self {
        Self {
            git_worktree,
            worktree_dir,
            worktree_session_dir,
            worktree_base_dir,
        }
    }
}

impl Drop for WorktreeGuard<'_> {
    fn drop(&mut self) {
        let _ = self.git_worktree.remove_force(self.worktree_dir);
        let _ = fs::remove_dir(self.worktree_session_dir);
        let _ = fs::remove_dir(self.worktree_base_dir);
    }
}

struct FilesCopyInner<'a> {
    /// The files to copy
    target_files: &'a [String],

    /// The root path to copy files from
    source_root: &'a Path,

    /// The path to the directory for output
    output_dir: &'a Path,
}

impl<'a> FilesCopyInner<'a> {
    fn new(target_files: &'a [String], source_root: &'a Path, output_dir: &'a Path) -> Self {
        Self {
            target_files,
            source_root,
            output_dir,
        }
    }

    fn copy<W: Write>(&self, w: &mut W) -> Result<()> {
        for file in self.target_files.iter() {
            let mut dir = PathBuf::from(file);
            dir.pop();
            let out_dir = self.output_dir.join(dir);
            fs::create_dir_all(&out_dir)?;
            let source_file = self.source_root.join(file);
            if source_file.is_file() {
                let dest_file = self.output_dir.join(file);
                fs::copy(&source_file, &dest_file)?;
                writeln!(
                    w,
                    "Copied: {} -> {}",
                    source_file.display(),
                    dest_file.display()
                )?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::git_test_lock;
    use flate2::read::GzDecoder;
    use outdir_tempdir::TempDir;
    use std::env;
    use std::fs::{self, File};
    use std::process::Command;
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

    struct TestRepo {
        dir: TempDir,
        repo_dir: PathBuf,
        output_dir: PathBuf,
        commit_a: String,
        commit_b: String,
        commit_c: String,
    }

    impl TestRepo {
        fn new() -> Self {
            let dir = TempDir::new().autorm();
            let repo_dir = dir.path().join("repo");
            let output_dir = dir.path().join("out");
            fs::create_dir_all(&repo_dir).unwrap();

            run_git(&repo_dir, &["init"]);
            run_git(&repo_dir, &["config", "user.name", "gde tests"]);
            run_git(
                &repo_dir,
                &["config", "user.email", "gde-tests@example.com"],
            );
            run_git(&repo_dir, &["config", "commit.gpgsign", "false"]);
            run_git(&repo_dir, &["config", "core.autocrlf", "false"]);

            write_bytes(
                repo_dir.join(".gitattributes"),
                b"crlf-normalized.txt text eol=crlf\n",
            );
            write_bytes(repo_dir.join("changed.txt"), b"before change\n");
            write_bytes(repo_dir.join("deleted.txt"), b"delete me\n");
            write_bytes(repo_dir.join("unchanged.txt"), b"stable\n");
            write_bytes(
                repo_dir.join("nested").join("path").join("file.txt"),
                b"nested a\n",
            );
            write_bytes(repo_dir.join("crlf.txt"), b"line1\r\nline2\r\n");
            write_bytes(repo_dir.join("crlf-normalized.txt"), b"line1\r\nline2\r\n");
            write_bytes(repo_dir.join("bin.dat"), &[0x00, 0x01, 0x02, 0x03, 0x0a]);
            let commit_a = commit_all(&repo_dir, "commit a");

            write_bytes(repo_dir.join("changed.txt"), b"after change\n");
            fs::remove_file(repo_dir.join("deleted.txt")).unwrap();
            write_bytes(repo_dir.join("added.txt"), b"added in commit b\n");
            write_bytes(
                repo_dir.join("nested").join("path").join("file.txt"),
                b"nested b\n",
            );
            write_bytes(repo_dir.join("crlf.txt"), b"line1\r\nline2 changed\r\n");
            write_bytes(
                repo_dir.join("crlf-normalized.txt"),
                b"line1\r\nline2 changed\r\n",
            );
            write_bytes(
                repo_dir.join("bin.dat"),
                &[0x00, 0xff, 0x10, 0x0d, 0x0a, 0x00, 0x7f],
            );
            let commit_b = commit_all(&repo_dir, "commit b");

            write_bytes(repo_dir.join("head-only.txt"), b"head commit only\n");
            let commit_c = commit_all(&repo_dir, "commit c");

            Self {
                dir,
                repo_dir,
                output_dir,
                commit_a,
                commit_b,
                commit_c,
            }
        }

        fn files_copy(&self, from: &str, to: &str) -> FilesCopy {
            FilesCopy::new(
                "git",
                from,
                to,
                &self.repo_dir,
                &self.output_dir,
                self.head(),
            )
        }

        fn files_copy_with_output_dir(
            &self,
            from: &str,
            to: &str,
            output_dir: impl Into<PathBuf>,
        ) -> FilesCopy {
            FilesCopy::new(
                "git",
                from,
                to,
                &self.repo_dir,
                output_dir.into(),
                self.head(),
            )
        }

        fn head(&self) -> String {
            run_git(&self.repo_dir, &["rev-parse", "HEAD"])
                .trim()
                .to_string()
        }

        fn output_file(&self, side: &str, path: &str) -> PathBuf {
            self.output_dir.join(side).join(path)
        }

        fn rev_file_bytes(&self, rev: &str, path: &str) -> Vec<u8> {
            git_show_bytes(&self.repo_dir, rev, path)
        }

        fn _keep_dir_alive(&self) -> &TempDir {
            &self.dir
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

    fn git_show_bytes(repo_dir: &Path, rev: &str, path: &str) -> Vec<u8> {
        let spec = format!("{rev}:{path}");
        let output = Command::new("git")
            .arg("show")
            .arg(spec)
            .current_dir(repo_dir)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "git show failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        output.stdout
    }

    fn write_bytes(path: impl AsRef<Path>, bytes: &[u8]) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    fn read_bytes(path: impl AsRef<Path>) -> Vec<u8> {
        fs::read(path).unwrap()
    }

    fn commit_all(repo_dir: &Path, message: &str) -> String {
        run_git(repo_dir, &["add", "-A"]);
        run_git(repo_dir, &["commit", "-m", message]);
        run_git(repo_dir, &["rev-parse", "HEAD"]).trim().to_string()
    }

    fn assert_file_bytes(path: impl AsRef<Path>, expected: &[u8]) {
        assert_eq!(read_bytes(path), expected);
    }

    fn assert_not_exists(path: impl AsRef<Path>) {
        assert!(
            !path.as_ref().exists(),
            "{} exists",
            path.as_ref().display()
        );
    }

    fn assert_exists(path: impl AsRef<Path>) {
        assert!(
            path.as_ref().exists(),
            "{} does not exist",
            path.as_ref().display()
        );
    }

    #[test]
    fn test_copy() {
        let _lock = git_test_lock();
        let dir = TempDir::new().autorm();
        let tempdir = dir.path();
        let f = File::open(get_test_file()).unwrap();
        let tar = GzDecoder::new(f);
        let mut archive = Archive::new(tar);
        archive.unpack(tempdir).unwrap();
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

    #[test]
    fn copy_outputs_changed_file_versions_to_from_and_to() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_file_bytes(
            repo.output_file("from", "changed.txt"),
            &repo.rev_file_bytes(&repo.commit_a, "changed.txt"),
        );
        assert_file_bytes(
            repo.output_file("to", "changed.txt"),
            &repo.rev_file_bytes(&repo.commit_b, "changed.txt"),
        );
    }

    #[test]
    fn copy_outputs_added_file_only_to_to_directory() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_not_exists(repo.output_file("from", "added.txt"));
        assert_file_bytes(
            repo.output_file("to", "added.txt"),
            &repo.rev_file_bytes(&repo.commit_b, "added.txt"),
        );
    }

    #[test]
    fn copy_outputs_deleted_file_only_to_from_directory() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_file_bytes(
            repo.output_file("from", "deleted.txt"),
            &repo.rev_file_bytes(&repo.commit_a, "deleted.txt"),
        );
        assert_not_exists(repo.output_file("to", "deleted.txt"));
    }

    #[test]
    fn copy_does_not_output_unchanged_files() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_not_exists(repo.output_file("from", "unchanged.txt"));
        assert_not_exists(repo.output_file("to", "unchanged.txt"));
    }

    #[test]
    fn copy_preserves_nested_directory_structure() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_file_bytes(
            repo.output_file("from", "nested/path/file.txt"),
            &repo.rev_file_bytes(&repo.commit_a, "nested/path/file.txt"),
        );
        assert_file_bytes(
            repo.output_file("to", "nested/path/file.txt"),
            &repo.rev_file_bytes(&repo.commit_b, "nested/path/file.txt"),
        );
    }

    #[test]
    fn copy_succeeds_with_empty_diff_and_reports_no_files() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_b, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("There are no files with differences"));
        assert_not_exists(repo.output_file("from", "changed.txt"));
        assert_not_exists(repo.output_file("to", "changed.txt"));
        assert_not_exists(repo.output_dir.join(".gde-worktrees"));
    }

    #[test]
    fn copy_succeeds_when_working_tree_has_unstaged_changes() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        write_bytes(repo.repo_dir.join("changed.txt"), b"dirty working tree\n");
        let before_head = repo.head();

        let mut out = Vec::new();
        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_eq!(before_head, repo.head());
        assert_file_bytes(repo.repo_dir.join("changed.txt"), b"dirty working tree\n");
        assert_eq!(
            "changed.txt",
            run_git(&repo.repo_dir, &["diff", "--name-only"]).trim()
        );
        assert_eq!(
            "",
            run_git(&repo.repo_dir, &["diff", "--staged", "--name-only"]).trim()
        );
        assert_file_bytes(
            repo.output_file("from", "changed.txt"),
            &repo.rev_file_bytes(&repo.commit_a, "changed.txt"),
        );
        assert_file_bytes(
            repo.output_file("to", "changed.txt"),
            &repo.rev_file_bytes(&repo.commit_b, "changed.txt"),
        );
    }

    #[test]
    fn copy_succeeds_when_working_tree_has_staged_changes() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        write_bytes(repo.repo_dir.join("changed.txt"), b"staged working tree\n");
        run_git(&repo.repo_dir, &["add", "changed.txt"]);
        let before_head = repo.head();

        let mut out = Vec::new();
        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_eq!(before_head, repo.head());
        assert_file_bytes(repo.repo_dir.join("changed.txt"), b"staged working tree\n");
        assert_eq!("", run_git(&repo.repo_dir, &["diff", "--name-only"]).trim());
        assert_eq!(
            "changed.txt",
            run_git(&repo.repo_dir, &["diff", "--staged", "--name-only"]).trim()
        );
        assert_file_bytes(
            repo.output_file("from", "changed.txt"),
            &repo.rev_file_bytes(&repo.commit_a, "changed.txt"),
        );
        assert_file_bytes(
            repo.output_file("to", "changed.txt"),
            &repo.rev_file_bytes(&repo.commit_b, "changed.txt"),
        );
    }

    #[test]
    fn copy_keeps_repository_head_after_completion() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let before = repo.head();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        let after = repo.head();
        assert_eq!(before, repo.commit_c);
        assert_eq!(after, before);
    }

    #[test]
    fn copy_leaves_working_tree_clean_after_completion() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_eq!("", run_git(&repo.repo_dir, &["diff", "--name-only"]).trim());
        assert_eq!(
            "",
            run_git(&repo.repo_dir, &["diff", "--staged", "--name-only"]).trim()
        );
        assert_not_exists(repo.output_dir.join(".gde-worktrees"));
    }

    #[test]
    fn copy_keeps_output_dir_after_worktree_cleanup() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_exists(&repo.output_dir);
        assert_exists(repo.output_file("from", "changed.txt"));
        assert_exists(repo.output_file("to", "changed.txt"));
        assert_not_exists(repo.output_dir.join(".gde-worktrees"));
    }

    #[test]
    fn copy_keeps_existing_output_dir_contents() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        write_bytes(repo.output_dir.join("keep.txt"), b"keep me\n");
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_file_bytes(repo.output_dir.join("keep.txt"), b"keep me\n");
        assert_exists(&repo.output_dir);
        assert_not_exists(repo.output_dir.join(".gde-worktrees"));
    }

    #[test]
    fn copy_preserves_checkout_crlf_bytes() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_eq!(
            repo.rev_file_bytes(&repo.commit_a, "crlf-normalized.txt"),
            b"line1\nline2\n"
        );
        assert_eq!(
            repo.rev_file_bytes(&repo.commit_b, "crlf-normalized.txt"),
            b"line1\nline2 changed\n"
        );
        assert_file_bytes(
            repo.output_file("from", "crlf-normalized.txt"),
            b"line1\r\nline2\r\n",
        );
        assert_file_bytes(
            repo.output_file("to", "crlf-normalized.txt"),
            b"line1\r\nline2 changed\r\n",
        );
    }

    #[test]
    fn copy_preserves_binary_file_bytes() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_file_bytes(
            repo.output_file("from", "bin.dat"),
            &repo.rev_file_bytes(&repo.commit_a, "bin.dat"),
        );
        assert_file_bytes(
            repo.output_file("to", "bin.dat"),
            &repo.rev_file_bytes(&repo.commit_b, "bin.dat"),
        );
    }

    #[test]
    fn copy_keeps_untracked_files_in_source_repository() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        write_bytes(repo.repo_dir.join("local-only.txt"), b"local only\n");
        let mut out = Vec::new();

        repo.files_copy(&repo.commit_a, &repo.commit_b)
            .copy(&mut out)
            .unwrap();

        assert_exists(repo.repo_dir.join("local-only.txt"));
        assert_not_exists(repo.output_file("from", "local-only.txt"));
        assert_not_exists(repo.output_file("to", "local-only.txt"));
    }

    #[test]
    fn copy_succeeds_with_output_dir_inside_repository() {
        let _lock = git_test_lock();
        let repo = TestRepo::new();
        let _ = repo._keep_dir_alive();
        let output_dir = repo.repo_dir.join("artifacts");
        let mut out = Vec::new();

        repo.files_copy_with_output_dir(&repo.commit_a, &repo.commit_b, &output_dir)
            .copy(&mut out)
            .unwrap();

        assert_exists(output_dir.join("from").join("changed.txt"));
        assert_exists(output_dir.join("to").join("changed.txt"));
        assert_not_exists(repo.repo_dir.join(".gde-worktrees"));
        assert_not_exists(output_dir.join(".gde-worktrees"));
    }
}
