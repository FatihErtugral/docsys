//! The one way the binary starts git (D-098).
//!
//! A git hook exports `GIT_DIR` for the repository it runs in, and git's rule
//! is that `GIT_DIR` without `GIT_WORK_TREE` makes the current directory the
//! top of the work tree. Inside a linked worktree's pre-commit hook,
//! `git -C docs rev-parse --show-toplevel` therefore answered `docs/`, every
//! pin path was looked up under it, and 37 pins read as moved. Only the pair
//! is an explicit configuration; a lone `GIT_DIR` is the hook's, and it is
//! dropped. `GIT_INDEX_FILE` stays: it is the index a partial commit writes,
//! and the commit gate must read that one.

use std::path::{Path, PathBuf};
use std::process::Command;

/// git, run in `dir`, for the repository the tree lives in. Paths come back
/// unquoted (D-048).
pub fn cmd(dir: &Path) -> Command {
    let mut c = Command::new("git");
    c.arg("-C").arg(dir).args(["-c", "core.quotePath=false"]);
    if std::env::var_os("GIT_WORK_TREE").is_none() {
        c.env_remove("GIT_DIR");
    }
    c
}

/// git for another repository — a provider being fetched, a project being
/// read, a repository being created — where nothing a calling hook exported
/// for its own repository may leak in.
pub fn foreign(dir: Option<&Path>) -> Command {
    let mut c = Command::new("git");
    if let Some(d) = dir {
        c.arg("-C").arg(d);
    }
    c.args(["-c", "core.quotePath=false"]);
    for var in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_NAMESPACE",
        "GIT_PREFIX",
    ] {
        c.env_remove(var);
    }
    c
}

/// The top level of the repository `dir` lives in; `None` outside any
/// repository (or when `dir` does not exist).
pub fn toplevel(dir: &Path) -> Option<PathBuf> {
    let out = cmd(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let top = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!top.is_empty()).then(|| PathBuf::from(top))
}

/// The directory git runs this repository's hooks from (D-100): git answers,
/// so `core.hooksPath` is honoured and a linked worktree — where `.git` is a
/// file — gets its common directory's hooks. A git older than 2.31 has no
/// `--path-format`; its answer is relative to `repo`.
pub fn hooks_dir(repo: &Path) -> Option<PathBuf> {
    let ask = |args: &[&str]| {
        cmd(repo)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    if let Some(p) = ask(&["rev-parse", "--path-format=absolute", "--git-path", "hooks"]) {
        return Some(PathBuf::from(p));
    }
    let p = PathBuf::from(ask(&["rev-parse", "--git-path", "hooks"])?);
    Some(if p.is_absolute() { p } else { repo.join(p) })
}

/// Blob contents by id, through one `git cat-file --batch` for the whole run:
/// history checks read a few blobs per page, and a process per blob would
/// make every lint pay for the size of the tree.
pub struct Blobs {
    child: std::process::Child,
    input: std::process::ChildStdin,
    output: std::io::BufReader<std::process::ChildStdout>,
}

impl Blobs {
    pub fn open(repo: &Path) -> Option<Blobs> {
        let mut child = cmd(repo)
            .args(["cat-file", "--batch"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;
        let input = child.stdin.take()?;
        let output = std::io::BufReader::new(child.stdout.take()?);
        Some(Blobs {
            child,
            input,
            output,
        })
    }

    /// The blob's text; `None` for an id git does not hold (or the all-zero
    /// id git writes for "no file on this side").
    pub fn read(&mut self, id: &str) -> Option<String> {
        use std::io::{BufRead, Read, Write};
        if id.is_empty() || id.bytes().all(|b| b == b'0') {
            return None;
        }
        writeln!(self.input, "{id}").ok()?;
        self.input.flush().ok()?;
        let mut header = String::new();
        self.output.read_line(&mut header).ok()?;
        let size: usize = header.trim_end().rsplit(' ').next()?.parse().ok()?;
        let mut bytes = vec![0u8; size + 1];
        self.output.read_exact(&mut bytes).ok()?;
        bytes.truncate(size);
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

impl Drop for Blobs {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // The hook environment itself (`GIT_DIR` exported by git) is reproduced
    // on the binary in tests/locate.rs; a unit test cannot set it for this
    // process without racing every other test.
    #[test]
    fn the_top_level_is_found_from_a_subdirectory_and_nothing_outside() {
        let base = std::env::temp_dir().join(format!("docsys-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("repo/docs/reference")).unwrap();
        assert!(Command::new("git")
            .args(["init", "-q"])
            .current_dir(base.join("repo"))
            .status()
            .unwrap()
            .success());
        let top = toplevel(&base.join("repo/docs/reference")).unwrap();
        assert_eq!(
            top.canonicalize().unwrap(),
            base.join("repo").canonicalize().unwrap()
        );
        assert_eq!(toplevel(&base.join("missing")), None);
        let _ = std::fs::remove_dir_all(&base);
    }
}
