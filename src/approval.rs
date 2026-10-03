//! A page's verification read from history on a docsys/0.5 tree (R-024,
//! D-126): verified when an approval for it lies between the commit that last
//! changed its body — its block sequence (D-103) — or a source it consumes,
//! and `HEAD`. An approval is a commit whose `Approved-by:` names a
//! maintainer and that changed the page or names it in `Verifies:`; a later
//! `Revokes:` takes it back. Nothing is written into the page, so nothing is
//! left to merge.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::fm::{Frontmatter, Value};
use crate::tree::{DocTree, Kind, Page, Profile};

/// The trailer a maintainer's word arrives in.
pub const APPROVED_BY: &str = "Approved-by";
/// The trailer that names the page an approval is for, when its commit
/// changed nothing — and, written by `docsys verify`, the body it read.
pub const VERIFIES: &str = "Verifies";
/// The trailer that takes an approval back.
pub const REVOKES: &str = "Revokes";

/// A page's verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// an approval follows the last change: who gave it, and in which commit
    /// (`record` for the frontmatter record a tree kept from before)
    Verified {
        by: String,
        commit: String,
    },
    Unverified,
    /// no history to read
    Unknown,
}

/// Whether a page takes part in verification (§3.2): every page of a
/// knowledge base, and a project page that carries `sources:` with something
/// to check its claims against — a source it names, or a pin to the code.
pub fn tracked(tree: &DocTree, page: &Page) -> bool {
    page.kind == Kind::Permanent
        && (tree.profile == Profile::KnowledgeBase
            || page.fm.as_ref().is_some_and(|f| {
                f.fields.get("sources").is_some_and(|s| match s {
                    Value::List(l) => !l.is_empty(),
                    Value::Str(s) => !s.trim().is_empty(),
                    Value::Maps(m) => !m.is_empty(),
                }) || !crate::fresh::pins_of(f).is_empty()
            }))
}

/// The values of every `<key>:` line of a message, after its subject.
fn trailer_values(message: &str, key: &str) -> Vec<String> {
    let prefix = format!("{key}:");
    message
        .lines()
        .skip(1)
        .filter_map(|l| l.trim_start().strip_prefix(&prefix))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

/// The maintainer an `Approved-by:` value names — `@login`, `handle` or
/// `handle <email>`, matched against `maintainers:` — or, with no maintainers
/// declared, the value itself (anyone, as before; R-208).
pub fn maintainer_of(maintainers: &[crate::checks::Maintainer], value: &str) -> Option<String> {
    let value = value.trim();
    if maintainers.is_empty() {
        return (!value.is_empty()).then(|| value.to_string());
    }
    let lower = value.to_lowercase();
    if let Some(login) = lower.strip_prefix('@') {
        return maintainers
            .iter()
            .find(|m| m.login_or_handle() == login.trim())
            .map(|m| m.handle.clone());
    }
    if let Some((_, email)) = lower.rsplit_once('<') {
        let email = email.trim_end_matches('>').trim();
        return maintainers
            .iter()
            .find(|m| m.email.as_deref() == Some(email))
            .map(|m| m.handle.clone());
    }
    let handle = crate::checks::record_handle(&lower);
    maintainers
        .iter()
        .find(|m| m.handle == handle)
        .map(|m| m.handle.clone())
}

/// The hash of a page's block sequence: the body a `Verifies:` value says
/// its approver read.
pub fn body_hash(text: &str) -> String {
    crate::blocks::short_hash(&crate::blocks::hashes(&crate::fresh::body_text(text)).join("\n"))
}

/// A `Verifies:` value: the page it names, and the body hash after it when
/// `docsys verify` wrote it.
fn verifies_value(value: &str) -> (String, Option<String>) {
    match value.trim().rsplit_once(' ') {
        Some((page, hash)) if hash.len() == 12 && hash.bytes().all(|b| b.is_ascii_hexdigit()) => {
            (page.trim().to_string(), Some(hash.to_string()))
        }
        _ => (value.trim().to_string(), None),
    }
}

/// A commit that speaks of verification: its trailers.
struct Act {
    /// the first-parent commit it counts at
    sha: String,
    approved_by: Vec<String>,
    /// the pages it names, each with the body hash its approver read, if named
    verifies: Vec<(String, Option<String>)>,
    revokes: Vec<String>,
    made: Made,
    /// its commit was made again after its author made it — a rebase, a
    /// cherry-pick, an amend: the tree it sits on is not the one they read
    remade: bool,
}

/// Where an act was made, when that is not the first-parent commit it counts
/// at: a commit a merge brought in, or one a squash message quotes.
enum Made {
    Here,
    /// the commit the approver read, and the paths it changed
    On {
        commit: String,
        touched: BTreeSet<String>,
    },
    /// quoted by a squash message, its commit no longer in the repository
    Unknown,
}

/// The acts a commit message carries: its own trailers, and those of each
/// commit a squash message quotes (`commit <sha>`, then the indented message).
fn acts_in(message: &str) -> Vec<(Option<String>, String)> {
    // each text starts with its subject, which `trailer_values` passes over
    let mut own = String::new();
    let mut quoted: Vec<(String, String)> = Vec::new();
    for line in message.lines() {
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if let Some(sha) = line
            .strip_prefix("commit ")
            .map(str::trim)
            .filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            quoted.push((sha.to_string(), String::new()));
        } else if let (true, Some((_, text))) = (indented, quoted.last_mut()) {
            text.push_str(line);
            text.push('\n');
        } else if !indented {
            own.push_str(line);
            own.push('\n');
        }
    }
    let mut out = vec![(None, own)];
    out.extend(quoted.into_iter().map(|(sha, text)| (Some(sha), text)));
    out
}

/// Every tracked page's state, from one walk of history.
pub struct Approvals {
    states: BTreeMap<String, State>,
    /// page → the newest approval of it anywhere in history: who, and the
    /// commit — what a re-verification compares against
    last: BTreeMap<String, (String, String)>,
    /// the pages an approval read before a source they consume moved (§13)
    sources_moved: BTreeSet<String>,
    repo: Option<std::path::PathBuf>,
    prefix: String,
}

impl Approvals {
    pub fn state(&self, rel: &str) -> State {
        self.states.get(rel).cloned().unwrap_or(State::Unverified)
    }

    /// The pages a maintainer approved before a source they consume moved.
    pub fn sources_moved(&self) -> usize {
        self.sources_moved.len()
    }

    /// Whether a source the page consumes moved since its approval.
    pub fn source_moved(&self, rel: &str) -> bool {
        self.sources_moved.contains(rel)
    }

    /// Whether a page takes part in verification and is not verified.
    pub fn is_unverified(&self, tree: &DocTree, page: &Page) -> bool {
        tracked(tree, page) && self.state(&page.rel) == State::Unverified
    }

    /// How much of a page reads as verified (R-212): the blocks the last
    /// approval read, found again, less those a stale bound pin backs — and
    /// who approved. `None` for a page never approved.
    pub fn reading(
        &self,
        page: &Page,
        stale: &[String],
    ) -> Option<(crate::blocks::Reading, String)> {
        let (by, sha) = self.last.get(&page.rel)?;
        let then = if matches!(self.state(&page.rel), State::Verified { .. }) {
            page.text.clone()
        } else {
            git_out(
                self.repo.as_deref()?,
                &["show", &format!("{sha}:{}{}", self.prefix, page.rel)],
            )?
        };
        let recorded = crate::blocks::hashes(&crate::fresh::body_text(&then));
        Some((
            crate::blocks::reading_of(&recorded, &page.text, stale),
            by.clone(),
        ))
    }

    /// The tracked pages that are not verified, by path.
    pub fn unverified(&self) -> Vec<&str> {
        self.states
            .iter()
            .filter(|(_, s)| !matches!(s, State::Verified { .. }))
            .map(|(rel, _)| rel.as_str())
            .collect()
    }

    pub fn of(tree: &DocTree) -> Approvals {
        let pages: Vec<&Page> = tree.pages.iter().filter(|p| tracked(tree, p)).collect();
        let Some(repo) = crate::repo_of(&tree.root) else {
            return Approvals {
                states: pages
                    .iter()
                    .map(|p| (p.rel.clone(), State::Unknown))
                    .collect(),
                last: BTreeMap::new(),
                sources_moved: BTreeSet::new(),
                repo: None,
                prefix: String::new(),
            };
        };
        let maintainers = crate::checks::maintainer_handles(tree);
        let prefix = match crate::fresh::root_rel(&repo, &tree.root) {
            r if r.is_empty() => String::new(),
            r => format!("{r}/"),
        };
        let history = walk(&repo, &prefix);
        let mut blobs = crate::git::Blobs::open(&repo);
        let mut states = BTreeMap::new();
        let mut last = BTreeMap::new();
        let mut sources_moved = BTreeSet::new();
        for page in pages {
            let fm = page.fm.as_ref();
            let path = format!("{prefix}{}", page.rel);
            // the window opens at the newest change of the body or of a source
            // the page consumes; an edit not yet committed opens it at HEAD
            let head_body = history
                .head_blobs
                .get(&path)
                .and_then(|b| blobs.as_mut().and_then(|bl| bl.read(b)));
            let edited = head_body.as_deref().is_none_or(|h| {
                crate::blocks::hashes(&crate::fresh::body_text(h))
                    != crate::blocks::hashes(&crate::fresh::body_text(&page.text))
            });
            let mut opens: Option<usize> = None;
            // a consumed source that moved since the body last changed opens
            // the window too; one fetched and not yet committed, at once
            let mut by_source = false;
            let mut source_edited = false;
            if !edited {
                for (at, old, new) in history.changes.get(&path).into_iter().flatten() {
                    let read = |b: &str, bl: &mut Option<crate::git::Blobs>| {
                        bl.as_mut()
                            .and_then(|x| x.read(b))
                            .map(|t| crate::blocks::hashes(&crate::fresh::body_text(&t)))
                    };
                    if read(old, &mut blobs) != read(new, &mut blobs) {
                        opens = Some(*at);
                        break;
                    }
                }
                for source in fm.map(crate::fresh::consumed_sources).unwrap_or_default() {
                    let Some((ns, id)) = source.trim_start_matches('@').split_once('/') else {
                        continue;
                    };
                    let prov = format!("{prefix}.federation/{ns}/{id}.provenance.yml");
                    let now = std::fs::read_to_string(repo.join(&prov)).ok();
                    let then = history
                        .head_blobs
                        .get(&prov)
                        .and_then(|b| blobs.as_mut().and_then(|bl| bl.read(b)));
                    if now != then {
                        source_edited = true;
                    }
                    if let Some((at, _, _)) = history.changes.get(&prov).and_then(|c| c.first()) {
                        if opens.is_none_or(|o| *at < o) {
                            by_source = true;
                        }
                        opens = Some(opens.map_or(*at, |o| o.min(*at)));
                    }
                }
            }
            if source_edited {
                opens = None;
                by_source = true;
            }
            let names = names_of(page);
            let decided = opens.and_then(|opens| {
                history
                    .acts
                    .iter()
                    .filter(|(at, _)| *at <= opens)
                    .find_map(|(at, act)| {
                        if act.revokes.iter().any(|v| names.contains(v.trim())) {
                            return Some(State::Unverified);
                        }
                        if !approves(&history, *at, act, &path, &names, &mut blobs) {
                            return None;
                        }
                        act.approved_by
                            .iter()
                            .find_map(|v| maintainer_of(&maintainers, v))
                            .map(|by| State::Verified {
                                by,
                                commit: act.sha.clone(),
                            })
                    })
            });
            let state = decided.unwrap_or_else(|| {
                if source_edited {
                    State::Unverified
                } else {
                    legacy(&tree.root, &maintainers, page, fm)
                }
            });
            // the newest approval anywhere: who, and the commit whose body it read
            let newest = history.acts.iter().find_map(|(at, act)| {
                let by = act
                    .approved_by
                    .iter()
                    .find_map(|v| maintainer_of(&maintainers, v))?;
                approves(&history, *at, act, &path, &names, &mut blobs).then(|| {
                    let read = match &act.made {
                        Made::On { commit, .. } => commit.clone(),
                        Made::Here | Made::Unknown => act.sha.clone(),
                    };
                    (by, read)
                })
            });
            if by_source && state == State::Unverified && newest.is_some() {
                sources_moved.insert(page.rel.clone());
            }
            if let Some(found) = newest {
                last.insert(page.rel.clone(), found);
            }
            states.insert(page.rel.clone(), state);
        }
        Approvals {
            states,
            last,
            sources_moved,
            repo: Some(repo),
            prefix,
        }
    }
}

/// Whether `act`, counted at first-parent position `at`, approves the body of
/// the page at `path` that landed there. One that names the body it read
/// counts where that body landed, whatever brought it in. One that names none
/// read the tree of the commit it was made on: it counts where that commit,
/// as its author made it, changed the page or names it — brought in from
/// another line only when the body there is the body that landed. A later
/// edit on the branch, or a concurrent one, outruns it.
fn approves(
    history: &History,
    at: usize,
    act: &Act,
    path: &str,
    names: &BTreeSet<String>,
    blobs: &mut Option<crate::git::Blobs>,
) -> bool {
    let mut body = |rev: &str| {
        blobs
            .as_mut()
            .and_then(|b| b.read(&format!("{rev}:{path}")))
            .map(|t| body_hash(&t))
    };
    let named = act.verifies.iter().find(|(v, _)| names.contains(v));
    if let Some((_, Some(read))) = named {
        return body(&act.sha).as_ref() == Some(read);
    }
    if act.remade {
        return false;
    }
    let named = named.is_some();
    let landed_here = history
        .touched
        .get(&at)
        .is_some_and(|paths| paths.contains(path));
    match &act.made {
        Made::Here => landed_here || named,
        Made::Unknown => named && !landed_here,
        Made::On { commit, touched } => {
            (touched.contains(path) || named)
                && matches!((body(commit), body(&act.sha)), (Some(read), Some(landed)) if read == landed)
        }
    }
}

/// The names a `Verifies:` or `Revokes:` value may give a page: its path, its
/// path without `.md`, its id.
fn names_of(page: &Page) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    names.insert(page.rel.clone());
    names.insert(page.rel.trim_end_matches(".md").to_string());
    if let Some(id) = page
        .fm
        .as_ref()
        .and_then(|f| f.fields.get("id"))
        .and_then(Value::as_str)
    {
        names.insert(id.to_string());
    }
    names
}

/// A record a tree kept from before (D-101): valid evidence while its blocks
/// are the body's, each source it consumes hashes as recorded, and its
/// verifier is a maintainer.
fn legacy(
    root: &Path,
    maintainers: &[crate::checks::Maintainer],
    page: &Page,
    fm: Option<&Frontmatter>,
) -> State {
    let Some(fm) = fm else {
        return State::Unverified;
    };
    if fm.fields.get("verification").and_then(Value::as_str) != Some("verified")
        || crate::blocks::holds(fm, &page.text) != Some(true)
    {
        return State::Unverified;
    }
    let recorded: Vec<(String, String)> = fm
        .fields
        .get("verified_sources")
        .and_then(Value::as_maps)
        .map(|maps| {
            maps.iter()
                .filter_map(|m| Some((m.get("source")?.clone(), m.get("hash")?.clone())))
                .collect()
        })
        .unwrap_or_default();
    let sources_hold = crate::fresh::consumed_sources(fm).iter().all(|s| {
        let now = crate::fresh::source_hash(root, s);
        recorded
            .iter()
            .any(|(src, then)| src == s && Some(then) == now.as_ref())
    });
    if !sources_hold {
        return State::Unverified;
    }
    fm.fields
        .get("verified_by")
        .and_then(Value::as_str)
        .and_then(|v| maintainer_of(maintainers, v))
        .map_or(State::Unverified, |by| State::Verified {
            by,
            commit: "record".to_string(),
        })
}

/// What a reader is told of a page's verification on a docsys/0.5 tree: `None`
/// when it is verified throughout or takes no part, else the state in words
/// and how much of it still reads as verified (R-212). `stale` holds the
/// blocks a stale bound pin backs.
pub fn caveat(
    tree: &DocTree,
    approvals: &Approvals,
    page: &Page,
    stale: &[String],
) -> Option<String> {
    if !tracked(tree, page) {
        return None;
    }
    let partial = approvals
        .reading(page, stale)
        .filter(|(r, _)| r.partial())
        .map(|(r, by)| format!(" — {}/{} blocks as verified by {by}", r.found, r.of))
        .unwrap_or_default();
    match approvals.state(&page.rel) {
        State::Verified { .. } if partial.is_empty() => None,
        State::Verified { .. } => Some(format!("verified{partial}")),
        State::Unverified => Some(format!("unverified{partial}")),
        State::Unknown => Some("verification unknown".to_string()),
    }
}

/// Whether a page carries a record from before that is still valid evidence.
pub fn holding_record(tree: &DocTree, page: &Page) -> bool {
    matches!(
        legacy(
            &tree.root,
            &crate::checks::maintainer_handles(tree),
            page,
            page.fm.as_ref()
        ),
        State::Verified { .. }
    )
}

/// The newest approval of a page anywhere in history, as `verify --show`
/// tells it.
pub struct LastApproval {
    pub by: String,
    /// the commit it was made on, short — a branch's own, not the merge that
    /// brought it in
    pub commit: String,
    /// the page as its approver read it, where history holds that
    pub read: Option<String>,
    /// why it no longer counts, when that is not the body or a source moving
    /// since
    pub lost: Option<String>,
}

/// The commit an act was made on.
fn made_commit(act: &Act) -> &str {
    match &act.made {
        Made::On { commit, .. } => commit,
        Made::Here | Made::Unknown => &act.sha,
    }
}

pub fn last_approval(tree: &DocTree, page: &Page) -> Option<LastApproval> {
    let repo = crate::repo_of(&tree.root)?;
    let maintainers = crate::checks::maintainer_handles(tree);
    let prefix = match crate::fresh::root_rel(&repo, &tree.root) {
        r if r.is_empty() => String::new(),
        r => format!("{r}/"),
    };
    let path = format!("{prefix}{}", page.rel);
    let history = walk(&repo, &prefix);
    let names = names_of(page);
    let landed_here = |at: &usize| {
        history
            .touched
            .get(at)
            .is_some_and(|paths| paths.contains(&path))
    };
    let (at, act, by) = history.acts.iter().find_map(|(at, act)| {
        let touched = matches!(&act.made, Made::On { touched, .. } if touched.contains(&path));
        if !landed_here(at) && !touched && !act.verifies.iter().any(|(v, _)| names.contains(v)) {
            return None;
        }
        let by = act
            .approved_by
            .iter()
            .find_map(|v| maintainer_of(&maintainers, v))?;
        Some((*at, act, by))
    })?;
    let short = |sha: &str| sha.chars().take(7).collect::<String>();
    let hash = act
        .verifies
        .iter()
        .find(|(v, _)| names.contains(v))
        .and_then(|(_, h)| h.clone());
    let text = (!matches!(act.made, Made::Unknown))
        .then(|| git_out(&repo, &["show", &format!("{}:{path}", made_commit(act))]))
        .flatten();
    // the body it read: the one its hash names, or its commit's as made
    let read = match &hash {
        Some(h) => text
            .filter(|t| body_hash(t) == *h)
            .or_else(|| (body_hash(&page.text) == *h).then(|| page.text.clone())),
        None if act.remade => None,
        None => text,
    };
    let revoked = history.acts.iter().find(|(a, r)| {
        *a <= at && !std::ptr::eq(r, act) && r.revokes.iter().any(|v| names.contains(v.trim()))
    });
    let lost = if let Some((_, r)) = revoked {
        Some(format!("taken back by {}", short(made_commit(r))))
    } else if hash.is_some() && read.is_none() {
        Some("the body that landed is not the body it read: an edit landed beside it".into())
    } else if act.remade && hash.is_none() {
        Some("a rebase, a cherry-pick or an amend made its commit again, and it names no body: what it read is unknown".into())
    } else if matches!(act.made, Made::Unknown) && landed_here(&at) {
        Some("the squash that quotes it changed the page, and the commit it quotes is gone: what it read is unknown".into())
    } else {
        None
    };
    Some(LastApproval {
        by,
        commit: short(made_commit(act)),
        read,
        lost,
    })
}

/// What history says, once.
#[derive(Default)]
struct History {
    /// repo path → (first-parent position, old blob, new blob), newest first
    changes: BTreeMap<String, Vec<(usize, String, String)>>,
    /// first-parent position → the repo paths that commit changed
    touched: BTreeMap<usize, BTreeSet<String>>,
    /// the commits that approve or revoke, newest first
    acts: Vec<(usize, Act)>,
    /// repo path → its blob at HEAD
    head_blobs: BTreeMap<String, String>,
}

fn git_out(repo: &Path, args: &[&str]) -> Option<String> {
    crate::git::cmd(repo)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The first-parent commit that brought `sha` in: the oldest on the line
/// that has it as an ancestor (`line` is newest first).
fn landing(repo: &Path, sha: &str, line: &[&str]) -> Option<usize> {
    let has = |i: usize| {
        line.get(i).is_some_and(|tip| {
            crate::git::cmd(repo)
                .args(["merge-base", "--is-ancestor", sha, tip])
                .status()
                .is_ok_and(|s| s.success())
        })
    };
    if !has(0) {
        return None;
    }
    let (mut lo, mut hi) = (0usize, line.len());
    // has(lo) holds; has(hi) does not (or hi is past the line)
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if has(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}

/// Whether a commit's `%at %ct` say it was made again after its author made it.
fn remade(dates: &str) -> bool {
    let mut d = dates.split_whitespace().map(|n| n.parse::<i64>().ok());
    matches!((d.next().flatten(), d.next().flatten()), (Some(at), Some(ct)) if ct > at)
}

/// An act made on `commit`: the paths that commit changed.
fn made_on(repo: &Path, commit: &str) -> Made {
    let touched = git_out(
        repo,
        &[
            "diff-tree",
            "--no-commit-id",
            "-r",
            "--name-only",
            "--no-renames",
            commit,
        ],
    )
    .map(|out| out.lines().map(str::to_string).collect())
    .unwrap_or_default();
    Made::On {
        commit: commit.to_string(),
        touched,
    }
}

fn walk(repo: &Path, prefix: &str) -> History {
    let mut h = History::default();
    let Some(order) = git_out(repo, &["rev-list", "--first-parent", "HEAD"]) else {
        return h;
    };
    let position: BTreeMap<&str, usize> = order
        .lines()
        .enumerate()
        .map(|(i, sha)| (sha.trim(), i))
        .collect();
    let scope = if prefix.is_empty() {
        ".".to_string()
    } else {
        prefix.trim_end_matches('/').to_string()
    };
    if let Some(log) = git_out(
        repo,
        &[
            "log",
            "--first-parent",
            "--diff-merges=first-parent",
            "--format=@@%H",
            "--raw",
            "--no-abbrev",
            "--no-renames",
            "--",
            &scope,
        ],
    ) {
        let mut at = None;
        for line in log.lines() {
            if let Some(sha) = line.strip_prefix("@@") {
                at = position.get(sha.trim()).copied();
            } else if let (Some(at), Some((meta, path))) =
                (at, line.strip_prefix(':').and_then(|l| l.split_once('\t')))
            {
                let ids: Vec<&str> = meta.split(' ').collect();
                if let (Some(old), Some(new)) = (ids.get(2), ids.get(3)) {
                    h.changes.entry(path.to_string()).or_default().push((
                        at,
                        old.to_string(),
                        new.to_string(),
                    ));
                    h.touched.entry(at).or_default().insert(path.to_string());
                }
            }
        }
    }
    // every commit HEAD reaches, not only the first-parent line: an approval
    // made on a branch a merge brought in, or quoted (indented) by a squash
    // message, counts where it landed
    let grep_a = format!("--grep=^[[:space:]]*{APPROVED_BY}:");
    let grep_r = format!("--grep=^[[:space:]]*{REVOKES}:");
    if let Some(log) = git_out(
        repo,
        &["log", "--format=%H%x1f%at %ct%x1f%B%x1e", &grep_a, &grep_r],
    ) {
        let first_parent: Vec<&str> = order.lines().map(str::trim).collect();
        for rec in log.split('\u{1e}') {
            let mut fields = rec.trim_start_matches('\n').splitn(3, '\u{1f}');
            let (Some(sha), Some(dates), Some(msg)) = (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let sha = sha.trim();
            let (at, here) = match position.get(sha) {
                Some(at) => (*at, true),
                None => match landing(repo, sha, &first_parent) {
                    Some(at) => (at, false),
                    None => continue,
                },
            };
            let Some(landed) = first_parent.get(at) else {
                continue;
            };
            for (quoted, text) in acts_in(msg) {
                let (made, remade) = match (quoted.as_deref(), here) {
                    (None, true) => (Made::Here, remade(dates)),
                    (None, false) => (made_on(repo, sha), remade(dates)),
                    (Some(inner), _) => {
                        match git_out(repo, &["log", "-1", "--format=%at %ct", inner]) {
                            Some(dates) => (made_on(repo, inner), remade(&dates)),
                            None => (Made::Unknown, false),
                        }
                    }
                };
                let act = Act {
                    sha: (*landed).to_string(),
                    approved_by: trailer_values(&text, APPROVED_BY),
                    verifies: trailer_values(&text, VERIFIES)
                        .iter()
                        .map(|v| verifies_value(v))
                        .collect(),
                    revokes: trailer_values(&text, REVOKES),
                    made,
                    remade,
                };
                if !act.approved_by.is_empty() || !act.revokes.is_empty() {
                    h.acts.push((at, act));
                }
            }
        }
        // newest first, as the first-parent line orders them
        h.acts.sort_by_key(|(at, _)| *at);
    }
    if let Some(tree) = git_out(repo, &["ls-tree", "-r", "HEAD", "--", &scope]) {
        for line in tree.lines() {
            if let Some((meta, path)) = line.split_once('\t') {
                if let Some(blob) = meta.split(' ').nth(2) {
                    h.head_blobs.insert(path.to_string(), blob.to_string());
                }
            }
        }
    }
    h
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn m(handle: &str, email: Option<&str>, login: Option<&str>) -> crate::checks::Maintainer {
        crate::checks::Maintainer {
            handle: handle.into(),
            email: email.map(str::to_string),
            login: login.map(str::to_string),
        }
    }

    #[test]
    fn an_approval_names_a_maintainer_by_login_email_or_handle() {
        let list = [m("ayse", Some("ayse@example.com"), Some("ayse-gh"))];
        assert_eq!(maintainer_of(&list, "@Ayse-GH").as_deref(), Some("ayse"));
        assert_eq!(
            maintainer_of(&list, "Ayşe <AYSE@example.com>").as_deref(),
            Some("ayse")
        );
        assert_eq!(maintainer_of(&list, "ayse").as_deref(), Some("ayse"));
        assert_eq!(maintainer_of(&list, "@mallory"), None);
        assert_eq!(maintainer_of(&[], "@anyone").as_deref(), Some("@anyone"));
    }

    #[test]
    fn trailers_are_read_after_the_subject() {
        let msg = "Verifies: not this\n\nVerifies: reference/a\nApproved-by: @ayse\n";
        assert_eq!(trailer_values(msg, VERIFIES), ["reference/a"]);
        assert_eq!(trailer_values(msg, APPROVED_BY), ["@ayse"]);
    }
}
