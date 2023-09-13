// TODO: Need refactoring
use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use gde::git::OnelineLog;
use gde::FilesCopy;
use ratatui::prelude::CrosstermBackend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;
use std::env;
use std::fmt::Display;
use std::io::{self, stdout, BufWriter, Stdout};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
struct Cli {
    /// Path to Git executable used when Git is not in the system PATH
    #[arg(long, value_name = "GIT EXECUTABLE")]
    git: Option<PathBuf>,

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

/// Create a string for display on the terminal
fn to_term_string(log: &OnelineLog, mark: Option<&str>) -> String {
    match mark {
        Some(mark) => format!("[{mark}] {log}"),
        None => format!("    {log}"),
    }
}

struct StatefullTermOnelineLog {
    state: ListState,
    items: Vec<OnelineLog>,
}

impl StatefullTermOnelineLog {
    fn new(items: Vec<OnelineLog>) -> Self {
        Self {
            state: ListState::default(),
            items,
        }
    }

    fn next(&mut self) {
        let i = if let Some(i) = self.state.selected() {
            self.get_next(i + 1)
        } else {
            self.get_next(0)
        };
        self.state.select(Some(i));
    }

    /// Get index of next commit(skip tree-branches)
    fn get_next(&mut self, i: usize) -> usize {
        if let Some(item) = self.items.get(i) {
            if let OnelineLog::Commit(_) = &item {
                return i;
            } else if i + 1 < self.items.len() {
                return self.get_next(i + 1);
            }
        }

        self.get_next(0)
    }

    fn prev(&mut self) {
        let i = if let Some(i) = self.state.selected() {
            if i > 0 {
                self.get_prev(i - 1)
            } else {
                self.get_prev(self.items.len() - 1)
            }
        } else {
            self.get_prev(0)
        };
        self.state.select(Some(i));
    }

    /// Get index of previous commit(skip tree-branches)
    fn get_prev(&mut self, i: usize) -> usize {
        if let Some(item) = self.items.get(i) {
            if let OnelineLog::Commit(_) = &item {
                return i;
            } else if i > 0 {
                return self.get_prev(i - 1);
            }
        }

        self.get_prev(0)
    }

    fn current(&self) -> Option<&OnelineLog> {
        if let Some(i) = self.state.selected() {
            Some(&self.items[i])
        } else {
            None
        }
    }
}

struct GdeTerminal {
    inner: Terminal<CrosstermBackend<Stdout>>,
    is_restored: bool,
}

impl GdeTerminal {
    pub fn new() -> Result<Self> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        Ok(Self {
            inner: Terminal::new(CrosstermBackend::new(stdout))?,
            is_restored: false,
        })
    }

    pub fn restore_terminal(&mut self) -> Result<()> {
        if self.is_restored {
            Ok(())
        } else {
            self.is_restored = true;
            disable_raw_mode()?;
            execute!(self.inner.backend_mut(), LeaveAlternateScreen,)?;
            Ok(self.inner.show_cursor()?)
        }
    }

    pub fn run(&mut self, commits: Vec<OnelineLog>) -> Result<Option<(String, String)>> {
        #[derive(PartialEq, Eq)]
        struct CommitInfo {
            hash: String,
            message: String,
        }

        impl CommitInfo {
            fn new(hash: impl Into<String>, message: impl Into<String>) -> Self {
                Self {
                    hash: hash.into(),
                    message: message.into(),
                }
            }
        }

        impl Display for CommitInfo {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{} - {}", self.hash, self.message)
            }
        }

        fn to_notice_string(v: Option<&CommitInfo>) -> String {
            match v {
                Some(v) => format!("{v}"),
                None => "".into(),
            }
        }

        let mut sl = StatefullTermOnelineLog::new(commits);
        sl.next();
        let mut from_commit: Option<CommitInfo> = None;
        let mut to_commit: Option<CommitInfo> = None;
        let mut notice_msg: Option<String> = None;
        'outer: loop {
            let logs = sl
                .items
                .iter()
                .map(|x| {
                    if let OnelineLog::Commit(y) = x {
                        if let Some(ref z) = from_commit {
                            if y.hash() == z.hash {
                                if from_commit == to_commit {
                                    return ListItem::new(to_term_string(x, Some("*")));
                                } else {
                                    return ListItem::new(to_term_string(x, Some("F")));
                                }
                            }
                        }
                        if let Some(ref z) = to_commit {
                            if y.hash() == z.hash {
                                return ListItem::new(to_term_string(x, Some("T")));
                            }
                        }

                        return ListItem::new(to_term_string(x, Some(" ")));
                    }
                    return ListItem::new(to_term_string(x, None));
                })
                .collect::<Vec<_>>();
            let logs = List::new(logs)
                .block(Block::default().title("commits").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
                .highlight_symbol("> ");

            let selected_commits = Block::new()
                .borders(Borders::ALL)
                .title("Selected commits (Press \"f\" to select it as the \"From Commit\". Press \"t\" to select it as the \"To Commit\".)");
            let disp_text = format!(
                "From: {}\nTo  : {}",
                to_notice_string(from_commit.as_ref()),
                to_notice_string(to_commit.as_ref())
            );
            let selected_commits = Paragraph::new(disp_text).block(selected_commits);

            let notice = Block::new().borders(Borders::ALL).title("Message");
            let notice_text = notice_msg
                .as_ref()
                .map(|x| x as &str)
                .unwrap_or_default()
                .to_string();
            let notice = Paragraph::new(notice_text).block(notice);
            self.inner.draw(|frame| {
                let mut log_size = frame.size();
                log_size.height -= 7;
                let mut sc_size = frame.size();
                sc_size.y += log_size.height;
                sc_size.height = 4;
                let mut notice_size = frame.size();
                notice_size.y += sc_size.y + sc_size.height;
                notice_size.height = 3;

                frame.render_stateful_widget(logs, log_size, &mut sl.state);
                frame.render_widget(selected_commits, sc_size);
                frame.render_widget(notice, notice_size);
            })?;

            while let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    (KeyCode::Esc, _) => break 'outer,
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => break 'outer,
                    (KeyCode::Enter, KeyModifiers::NONE) => match (&from_commit, &to_commit) {
                        (Some(from), Some(to)) => {
                            if from.hash == to.hash {
                                notice_msg = Some("Select different commits for \"From Commit\" and \"To Commit\"".into());
                                continue 'outer;
                            } else {
                                break 'outer;
                            }
                        }
                        (Some(_), None) => {
                            notice_msg = Some("\"To Commit\" is not selected".into());
                            continue 'outer;
                        }
                        (None, Some(_)) => {
                            notice_msg = Some("\"From Commit\" is not selected".into());
                            continue 'outer;
                        }

                        (None, None) => {
                            notice_msg =
                                Some("\"From Commit\" and \"To Commit\" are not selected".into());
                            continue 'outer;
                        }
                    },
                    (KeyCode::Down, KeyModifiers::NONE) => {
                        sl.next();
                        continue 'outer;
                    }
                    (KeyCode::Up, KeyModifiers::NONE) => {
                        sl.prev();
                        continue 'outer;
                    }
                    (KeyCode::Char('f'), KeyModifiers::NONE) => {
                        if let Some(OnelineLog::Commit(ref c)) = sl.current() {
                            from_commit = Some(CommitInfo::new(c.hash(), c.message()));
                            notice_msg = Some(format!("Selected {} as \"From Commit\"", c.hash()));
                        } else {
                            from_commit = None;
                            notice_msg = Some("Cleared the \"From Commit\"".to_string());
                        }
                        continue 'outer;
                    }
                    (KeyCode::Char('t'), KeyModifiers::NONE) => {
                        if let Some(OnelineLog::Commit(ref c)) = sl.current() {
                            to_commit = Some(CommitInfo::new(c.hash(), c.message()));
                            notice_msg = Some(format!("Selected {} as \"To Commit\"", c.hash()));
                        } else {
                            to_commit = None;
                            notice_msg = Some("Cleared the \"To Commit\"".to_string());
                        }
                        continue 'outer;
                    }
                    _ => (),
                }
            }
        }

        match (from_commit, to_commit) {
            (Some(f), Some(t)) => Ok(Some((f.hash, t.hash))),
            _ => Ok(None),
        }
    }
}

impl Drop for GdeTerminal {
    fn drop(&mut self) {
        self.restore_terminal().unwrap();
    }
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
    let gitlog = gde::git::GitLog::new(&git_path, true, &target_dir)?;
    let logs = gitlog.tree()?;
    let logs = logs.into_iter().map(OnelineLog::from).collect::<Vec<_>>();
    let mut term = GdeTerminal::new()?;
    let selected = term.run(logs)?;
    term.restore_terminal()?;

    if let Some((from, to)) = selected {
        let mut output_dir = if let Some(dir) = cli.output {
            absolute_path(dir)?
        } else {
            env::current_dir()?
        };
        output_dir.push(format!("gde-{}", uuid::Uuid::new_v4()));
        let current_commit = git.get_hash(&target_dir, "HEAD")?;
        let f = FilesCopy::new(
            &git_path,
            from,
            to,
            &target_dir,
            &output_dir,
            current_commit,
        );
        let out = stdout();
        let mut out = BufWriter::new(out.lock());
        f.copy(&mut out)?;
    }
    Ok(())
}
