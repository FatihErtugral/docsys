//! Where a command works (D-098): the tree is found from where the command
//! stands, and the repository comes from the tree — never from the working
//! directory, never from a calling git hook's environment.
//!
//! Field report: a session in a subdirectory broke all four relays at once —
//! the commit gate blocked every commit on R-160, the `updated:` bump did
//! nothing and said nothing, the first-turn digest counted zero pages, and the
//! end-of-turn reminder claimed code changed without documentation while the
//! page sat in the same change.

use crate::git;
use std::path::{Path, PathBuf};

/// The tree and the repository a command operates on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub root: PathBuf,
    /// The top level of the tree's repository; `None` outside one.
    pub repo: Option<PathBuf>,
}

fn has_docmeta(dir: &Path) -> bool {
    dir.join(".docmeta.yml").is_file()
}

/// The tree a relative `root` names, seen from `anchor`: from the anchor up to
/// its repository's top level, the first directory `D` where
/// `D/<root>/.docmeta.yml` or `D/.docmeta.yml` exists. Outside a repository
/// only the anchor itself is tried — a stray `.docmeta.yml` higher on the
/// disk is never adopted.
pub fn find_tree(anchor: &Path, root: &Path) -> Option<PathBuf> {
    if root.is_absolute() {
        return has_docmeta(root).then(|| root.to_path_buf());
    }
    let start = anchor.canonicalize().ok()?;
    let top = git::toplevel(&start).and_then(|t| t.canonicalize().ok());
    let mut dir = start.as_path();
    loop {
        let named = dir.join(root);
        if has_docmeta(&named) {
            return Some(named);
        }
        if has_docmeta(dir) {
            return Some(dir.to_path_buf());
        }
        match &top {
            Some(t) if dir != t.as_path() && dir.starts_with(t) => dir = dir.parent()?,
            _ => return None,
        }
    }
}

/// The repository's one tree: the directory of the only `.docmeta.yml` git
/// sees under `top`, tracked or not ignored. `None` for none, or for several —
/// which of them a command means is not guessed.
pub fn only_tree(top: &Path) -> Option<PathBuf> {
    let out = git::cmd(top)
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            ":(glob)**/.docmeta.yml",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let found: std::collections::BTreeSet<&str> = text.lines().collect();
    match found.into_iter().collect::<Vec<_>>().as_slice() {
        [one] => Some(top.join(one).parent()?.to_path_buf()),
        _ => None,
    }
}

/// `path` as the working directory sees it: relative when it lies under the
/// working directory (so a command run from the top level prints what it
/// always printed), absolute otherwise.
pub fn rel_to_cwd(path: &Path) -> PathBuf {
    if path.is_relative() {
        return path.to_path_buf();
    }
    let (Ok(cwd), Ok(p)) = (
        std::env::current_dir().and_then(|c| c.canonicalize()),
        path.canonicalize(),
    ) else {
        return path.to_path_buf();
    };
    match p.strip_prefix(&cwd) {
        Ok(rest) if rest.as_os_str().is_empty() => PathBuf::from("."),
        Ok(rest) => rest.to_path_buf(),
        Err(_) => p,
    }
}

/// Keep the caller's own spelling when the found path is the one it named.
fn spelled(found: &Path, given: &Path) -> PathBuf {
    let same = match (found.canonicalize(), given.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if same {
        given.to_path_buf()
    } else {
        rel_to_cwd(found)
    }
}

/// The place of a command that operates on an existing tree. `anchors` are
/// tried in order (a hook passes the edited file's directory, the payload's
/// `cwd`, the process's, `$CLAUDE_PROJECT_DIR`); a given `--repo` is the
/// first anchor and the base of the fallback. When no tree is found the
/// literal path stands — relative to `--repo` when one is given, to the
/// working directory otherwise — and R-160 reports it, as before.
pub fn locate(anchors: &[PathBuf], root: &Path, repo: Option<&Path>) -> Place {
    let literal = match repo {
        Some(r) if root.is_relative() => r.join(root),
        _ => root.to_path_buf(),
    };
    let mut tried: Vec<&Path> = Vec::new();
    if let Some(r) = repo {
        tried.push(r);
    }
    tried.extend(anchors.iter().map(PathBuf::as_path));
    // nothing above: a repository with one tree is that tree's (D-098)
    let found = tried
        .iter()
        .find_map(|a| find_tree(a, root))
        .or_else(|| {
            tried
                .iter()
                .find_map(|a| git::toplevel(a))
                .and_then(|top| only_tree(&top))
        })
        .map(|f| spelled(&f, &literal))
        .unwrap_or(literal);
    let top = match repo {
        Some(r) => Some(match git::toplevel(r) {
            Some(t) => spelled(&t, r),
            None => r.to_path_buf(),
        }),
        None => git::toplevel(&found).map(|t| rel_to_cwd(&t)),
    };
    Place {
        root: found,
        repo: top,
    }
}

/// The anchors of a command run at a terminal: the working directory.
pub fn cwd_anchor() -> Vec<PathBuf> {
    std::env::current_dir().into_iter().collect()
}

/// The anchors of a hook (D-098): the edited file's own directory (it belongs
/// to the nearest tree above it), the payload's `cwd`, the process's working
/// directory, and `$CLAUDE_PROJECT_DIR` last.
pub fn hook_anchors(file: Option<&str>, payload_cwd: Option<&str>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = file
        .map(Path::new)
        .filter(|f| f.is_absolute())
        .and_then(Path::parent)
    {
        out.push(dir.to_path_buf());
    }
    if let Some(c) = payload_cwd.filter(|c| !c.is_empty()) {
        out.push(PathBuf::from(c));
    }
    out.extend(cwd_anchor());
    if let Some(p) = std::env::var_os("CLAUDE_PROJECT_DIR").filter(|p| !p.is_empty()) {
        out.push(PathBuf::from(p));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("docsys-place-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    fn init(dir: &Path) {
        assert!(Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn the_nearest_tree_above_the_anchor_answers() {
        let r = scratch("near");
        init(&r);
        fs::create_dir_all(r.join("docs")).unwrap();
        fs::write(r.join("docs/.docmeta.yml"), "spec: docsys/0.4\n").unwrap();
        fs::create_dir_all(r.join("apps/x/docs")).unwrap();
        fs::write(r.join("apps/x/docs/.docmeta.yml"), "spec: docsys/0.4\n").unwrap();
        fs::create_dir_all(r.join("apps/x/src")).unwrap();
        fs::create_dir_all(r.join("apps/y/src")).unwrap();
        fs::create_dir_all(r.join("docs/reference")).unwrap();
        let docs = Path::new("docs");
        assert_eq!(
            find_tree(&r.join("apps/x/src"), docs),
            Some(r.join("apps/x/docs"))
        );
        assert_eq!(find_tree(&r.join("apps/y/src"), docs), Some(r.join("docs")));
        assert_eq!(
            find_tree(&r.join("docs/reference"), docs),
            Some(r.join("docs"))
        );
        assert_eq!(find_tree(&r, docs), Some(r.join("docs")));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_base_at_the_top_level_is_found_with_the_default_root() {
        let r = scratch("base");
        init(&r);
        fs::write(r.join(".docmeta.yml"), "profile: knowledge-base\n").unwrap();
        fs::create_dir_all(r.join("wiki/coding")).unwrap();
        assert_eq!(
            find_tree(&r.join("wiki/coding"), Path::new("docs")),
            Some(r.clone())
        );
        assert_eq!(find_tree(&r, Path::new(".")), Some(r.join(".")));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn outside_a_repository_only_the_anchor_is_tried() {
        let r = scratch("stray");
        fs::create_dir_all(r.join("docs")).unwrap();
        fs::write(r.join("docs/.docmeta.yml"), "spec: docsys/0.4\n").unwrap();
        fs::create_dir_all(r.join("elsewhere/deep")).unwrap();
        assert_eq!(
            find_tree(&r.join("elsewhere/deep"), Path::new("docs")),
            None
        );
        assert_eq!(find_tree(&r, Path::new("docs")), Some(r.join("docs")));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn the_walk_stops_at_the_repository_top() {
        let r = scratch("bound");
        fs::create_dir_all(r.join("docs")).unwrap();
        fs::write(r.join("docs/.docmeta.yml"), "spec: docsys/0.4\n").unwrap();
        fs::create_dir_all(r.join("inner/src")).unwrap();
        init(&r.join("inner"));
        // the tree above the inner repository belongs to nobody in it
        assert_eq!(find_tree(&r.join("inner/src"), Path::new("docs")), None);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn nothing_found_leaves_the_literal_path_relative_to_the_repo() {
        let r = scratch("literal");
        let p = locate(&[r.join("nowhere")], Path::new("docs"), Some(&r));
        assert_eq!(p.root, r.join("docs"));
        let p = locate(&[r.join("nowhere")], Path::new("docs"), None);
        assert_eq!(p.root, PathBuf::from("docs"));
        assert_eq!(p.repo, None);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn hook_anchors_come_in_their_order() {
        let a = hook_anchors(Some("/x/docs/reference/a.md"), Some("/x/apps"));
        assert_eq!(a.first(), Some(&PathBuf::from("/x/docs/reference")));
        assert_eq!(a.get(1), Some(&PathBuf::from("/x/apps")));
        let b = hook_anchors(Some("relative.md"), None);
        assert!(!b.contains(&PathBuf::from("")));
    }
}
