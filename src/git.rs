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
