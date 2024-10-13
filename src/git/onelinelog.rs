use crate::git::Error;
use std::fmt::Display;
use std::str::FromStr;

/// The information for one line of the tree displayed by "git log --graph ..."
#[derive(Debug, Clone)]
pub enum OnelineLog {
    /// Commit information
    Commit(Commit),

    /// Tree branches only
    TreeBranches(String),
}

impl OnelineLog {
    /// Parse from `git log --graph --pretty=format:%h -%d %s (%ci) <%an> --abbrev-commit --date=relative`
    pub fn from(s: impl AsRef<str>) -> OnelineLog {
        let s = s.as_ref();
        if let Ok(commit) = Commit::from_str(s) {
            OnelineLog::Commit(commit)
        } else {
            OnelineLog::TreeBranches(s.to_string())
        }
    }
}

impl Display for OnelineLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Commit(commit) => write!(f, "{commit}"),
            Self::TreeBranches(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone)]
/// Information about a single-line commit from the tree displayed by "git log --graph ..."
pub struct Commit {
    /// Tree
    /// This refers to the '|\', '|', and '|/' in the following graph
    ///
    /// ```
    /// *   3706c44 - (origin/master, master) Merge pull request #1 from niumlaque/single-binary-for-windows (2023-08-15 12:52:59 +0900) <Niumlaque>
    /// |\  
    /// | * e252a0a - (origin/single-binary-for-windows, single-binary-for-windows) Add configuration to generate a single binary for Windows (2023-08-15 12:52:25 +0900) <Niumlaque>
    /// |/
    /// ```
    tree_head: String,

    /// Padding for hash
    hash_padding: String,

    /// Hash of commit
    hash: String,

    /// branch, tag, and so on...
    aliases: Option<String>,

    /// Commit message
    message: String,

    /// Commit date
    date: String,

    /// Author
    author: String,
}

impl Commit {
    fn new(
        tree_head: impl Into<String>,
        hash_padding: impl Into<String>,
        hash: impl Into<String>,
        aliases: Option<impl Into<String>>,
        message: impl Into<String>,
        date: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            tree_head: tree_head.into(),
            hash_padding: hash_padding.into(),
            hash: hash.into(),
            aliases: aliases.map(Into::into),
            message: message.into(),
            date: date.into(),
            author: author.into(),
        }
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tree_head)?;
        write!(f, "* ")?;
        write!(f, "{}", self.hash_padding)?;
        write!(f, "{}", self.hash)?;
        write!(f, " -")?;
        if let Some(aliases) = self.aliases.as_ref() {
            write!(f, " ({aliases})")?;
        }
        write!(f, " {}", self.message)?;
        write!(f, " ({})", self.date)?;
        write!(f, " <{}>", self.author)
    }
}

impl FromStr for Commit {
    type Err = Error;
    /// Parse from `git log --graph --pretty=format:%h -%d %s (%ci) <%an> --abbrev-commit --date=relative`
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let chars = s.chars().collect::<Vec<_>>();
        let r = RangeStr::new(chars);
        let hash_range = r
            .first_range('*', '-')
            .ok_or_else(|| Error::LogParse(s.to_string()))?;
        let hash = r.str_from_range(hash_range.0, hash_range.1);
        let padding = itertools::repeat_n(' ', hash.chars().take_while(|&x| x == ' ').count() - 1)
            .collect::<String>();
        let hash = hash.trim().to_string();
        let aliases_range = r
            .first_range('(', ')')
            .ok_or_else(|| Error::LogParse(s.to_string()))?;
        let aliases = r.str_from_range(aliases_range.0, aliases_range.1);
        let date_range = r
            .last_range('(', ')')
            .ok_or_else(|| Error::LogParse(s.to_string()))?;
        let date = r.str_from_range(date_range.0, date_range.1);
        let author = r
            .last_str('<', '>')
            .ok_or_else(|| Error::LogParse(s.to_string()))?;

        let (aliases, msgp1) = if aliases == date {
            (None, hash_range.1 + 1)
        } else {
            (Some(aliases), aliases_range.1 + 1)
        };
        let message = r.str_from_range(msgp1, date_range.0 - 1).trim().to_string();
        let tree_head = r.str_from_range(0, hash_range.0 - 1);

        Ok(Commit::new(
            tree_head, padding, hash, aliases, message, date, author,
        ))
    }
}

struct RangeStr {
    c: Vec<char>,
}

impl RangeStr {
    fn new(c: Vec<char>) -> Self {
        Self { c }
    }

    #[allow(dead_code)]
    fn first_str(&self, p1: char, p2: char) -> Option<String> {
        self.first_range(p1, p2)
            .map(|(p1, p2)| self.str_from_range(p1, p2))
    }

    fn last_str(&self, p1: char, p2: char) -> Option<String> {
        self.last_range(p1, p2)
            .map(|(p1, p2)| self.str_from_range(p1, p2))
    }

    fn str_from_range(&self, p1: usize, p2: usize) -> String {
        let (p1, p2) = if p1 <= p2 { (p1, p2) } else { (p2, p1) };
        self.c[p1..p2].iter().collect()
    }

    fn first_range(&self, p1: char, p2: char) -> Option<(usize, usize)> {
        let p1 = self.c.iter().position(|&x| x == p1);
        let p2 = self.c.iter().position(|&x| x == p2);
        match (p1, p2) {
            (Some(p1), Some(p2)) => Some((p1 + 1, p2)),
            _ => None,
        }
    }

    fn last_range(&self, p1: char, p2: char) -> Option<(usize, usize)> {
        let p1 = self.c.iter().rev().position(|&x| x == p1);
        let p2 = self.c.iter().rev().position(|&x| x == p2);
        match (p1, p2) {
            (Some(p1), Some(p2)) => Some((self.c.len() - p1, self.c.len() - p2 - 1)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_range() {
        let source = "| * e252a0a - (origin/single-binary-for-windows) Add configuration to generate a single binary for Windows (2023-08-15 12:52:25 +0900) <Niumlaque>";
        let chars = source.chars().collect::<Vec<_>>();
        let r = RangeStr::new(chars);
        assert_eq!(Some(" e252a0a "), r.first_str('*', '-').as_deref());
        assert_eq!(
            Some("origin/single-binary-for-windows"),
            r.first_str('(', ')').as_deref()
        );
        assert_eq!(
            Some("2023-08-15 12:52:25 +0900"),
            r.last_str('(', ')').as_deref()
        );
        assert_eq!(Some("Niumlaque"), r.last_str('<', '>').as_deref());

        let expected = "Add configuration to generate a single binary for Windows";
        let (_, ar1) = r.first_range('(', ')').unwrap();
        let (dr0, _) = r.last_range('(', ')').unwrap();
        let actual = r.str_from_range(ar1 + 1, dr0 - 1).trim().to_string();
        assert_eq!(expected, actual);

        let (hr0, _) = r.first_range('*', '-').unwrap();
        let tree_head = r.str_from_range(0, hr0 - 1);
        assert_eq!("| ", tree_head);
    }

    #[test]
    fn test_commit() {
        let source = "* 6d14782 - Initial commit (2023-08-06 23:23:20 +0900) <Niumlaque>";
        let c = Commit::from_str(source).unwrap();
        assert_eq!("", c.tree_head);
        assert_eq!("6d14782", c.hash);
        assert_eq!(None, c.aliases);
        assert_eq!("Initial commit", c.message);
        assert_eq!("2023-08-06 23:23:20 +0900", c.date);
        assert_eq!("Niumlaque", c.author);
        assert_eq!(source, c.to_string());

        let source = "| * e252a0a - (origin/single-binary-for-windows) Add configuration to generate a single binary for Windows (2023-08-15 12:52:25 +0900) <Niumlaque>";
        let c = Commit::from_str(source).unwrap();
        assert_eq!("| ", c.tree_head);
        assert_eq!("e252a0a", c.hash);
        assert_eq!(
            Some("origin/single-binary-for-windows"),
            c.aliases.as_deref()
        );
        assert_eq!(
            "Add configuration to generate a single binary for Windows",
            c.message
        );
        assert_eq!("2023-08-15 12:52:25 +0900", c.date);
        assert_eq!("Niumlaque", c.author);
        assert_eq!(source, c.to_string());

        let source = "*   3706c44 - (HEAD -> master, origin/master, origin/HEAD) )|-(()<\\>a><*---*( (2023-08-15 12:52:59 +0900) <Niumlaque>";
        let c = Commit::from_str(source).unwrap();
        assert_eq!("", c.tree_head);
        assert_eq!("3706c44", c.hash);
        assert_eq!(
            Some("HEAD -> master, origin/master, origin/HEAD"),
            c.aliases.as_deref()
        );
        assert_eq!(")|-(()<\\>a><*---*(", c.message);
        assert_eq!("2023-08-15 12:52:59 +0900", c.date);
        assert_eq!("Niumlaque", c.author);
        assert_eq!(source, c.to_string());
    }
}
