//! Which rules a tree answers to (D-118). A person installs one docsys and
//! works on trees at different spec versions, because repositories do not all
//! move on the same day. Each tree is judged, and written, by the spec it
//! declares: a docsys/0.4 tree gets the findings and the file formats 0.15.1
//! gave it, and every 0.5 rule or format is switched on here — by the declared
//! version, nowhere else — until `docsys upgrade` moves the tree on.

use std::path::Path;

/// The spec minor a tree declares (`spec: docsys/0.<n>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Era(pub u32);

/// Every tree written before 0.5 declares 0.4; one that declares nothing
/// readable is treated as it.
const FIRST: u32 = 4;
const V05: u32 = 5;

thread_local! {
    /// The era every gate answers with while `docsys upgrade` previews what a
    /// tree would be judged by after the move (D-117); unset otherwise.
    static PREVIEW: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}

/// Run `f` as if every tree declared `docsys/0.<minor>` — the upgrade's
/// preview of the findings the move adds and removes.
pub fn preview<T>(minor: u32, f: impl FnOnce() -> T) -> T {
    PREVIEW.with(|p| p.set(Some(minor)));
    let out = f();
    PREVIEW.with(|p| p.set(None));
    out
}

/// The minor a `spec:` value names (`docsys/0.<minor>`), quotes aside.
fn minor(spec: &str) -> Option<u32> {
    spec.trim()
        .trim_matches(['"', '\''])
        .strip_prefix("docsys/0.")
        .and_then(|m| m.parse().ok())
}

/// The version notice's words for a tree served by rules older than this
/// docsys's: what its `spec:` declares, in lint's terms (R-160, R-013), and
/// whose rules serve it. A tree read as docsys/0.4 for want of a version is
/// never said to declare one (D-118).
pub fn served(root: &Path) -> String {
    match declares(root) {
        Declares::Version(m) => format!("declares docsys/0.{m} and is served by its rules"),
        Declares::Other(s) => format!(
            "declares `{s}`, no `docsys/0.<minor>` version, and is served by docsys/0.{FIRST}'s rules"
        ),
        Declares::Nothing => format!("declares no spec and is served by docsys/0.{FIRST}'s rules"),
    }
}

/// What a tree's `spec:` declares, in lint's terms: `declares docsys/0.4`,
/// `declares no spec`, or the value that is no version.
pub fn declared(root: &Path) -> String {
    match declares(root) {
        Declares::Version(m) => format!("declares docsys/0.{m}"),
        Declares::Other(s) => format!("declares `{s}`, no `docsys/0.<minor>` version"),
        Declares::Nothing => "declares no spec".to_string(),
    }
}

enum Declares {
    Version(u32),
    Other(String),
    Nothing,
}

fn declares(root: &Path) -> Declares {
    let spec = crate::tree::docmeta_value(root, "spec");
    match spec.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => minor(s).map_or_else(|| Declares::Other(s.to_string()), Declares::Version),
        None => Declares::Nothing,
    }
}

impl Era {
    pub fn of_spec(spec: Option<&str>) -> Era {
        if let Some(m) = PREVIEW.with(std::cell::Cell::get) {
            return Era(m);
        }
        Era(spec.and_then(minor).unwrap_or(FIRST))
    }

    pub fn of(tree: &crate::tree::DocTree) -> Era {
        Era::of_spec(tree.docmeta_str("spec"))
    }

    /// The spec a tree that declares none is read as (D-118), as its
    /// `spec:` value.
    pub fn unstated_spec() -> String {
        format!("docsys/0.{FIRST}")
    }

    /// The era of the tree at `root`, read straight from its `.docmeta.yml` —
    /// for a write that has no loaded tree.
    pub fn at(root: &Path) -> Era {
        Era::of_spec(crate::tree::docmeta_value(root, "spec").as_deref())
    }

    fn v05(self) -> bool {
        self.0 >= V05
    }

    /// D-101: `verify` records the body's and the consumed sources' hashes,
    /// lint checks them without history, an edit demotes a verified page, a
    /// revoke keeps the record. Before: the record names who and which
    /// revision, and history is the only check.
    pub fn anchored_verification(self) -> bool {
        self.v05()
    }

    /// §2.4 for R-106 and R-085: a page's last change is its last CONTENT
    /// change. Before: any commit that touched the file.
    pub fn content_history(self) -> bool {
        self.v05()
    }

    /// D-106: a symbol resolves to its declaration, never to a use site;
    /// members resolve inside their owner. Before: D-069's whole-word line
    /// that opens a block.
    pub fn declaration_pins(self) -> bool {
        self.v05()
    }

    /// D-119: a pin is fresh while an acknowledgement of its region's token
    /// form exists under `.verifies/<page-id>/`; a refresh writes
    /// acknowledgements, never the page. Before: the region's canonical hash
    /// in the page's own `hash:` line, rewritten with `updated:` on every
    /// refresh.
    pub fn acknowledged_pins(self) -> bool {
        self.v05()
    }

    /// R-071: a wiki-link inside an inline code span is quoted material.
    /// Before: read as a link.
    pub fn literal_code_spans(self) -> bool {
        self.v05()
    }

    /// D-107: a code-side citation is `doc:` opening a line's text or right
    /// after a comment leader. Before: `doc:` anywhere on the line.
    pub fn positional_citations(self) -> bool {
        self.v05()
    }

    /// D-108: a ledger's field markers are ASCII ` -- `; an em dash makes the
    /// entry non-matching. Before: a spaced em dash was read as ` -- `.
    pub fn ascii_ledger(self) -> bool {
        self.v05()
    }

    /// R-208: a scalar `maintainers:` names nobody, and is reported.
    /// Before: read silently as an empty list.
    pub fn scalar_maintainers(self) -> bool {
        self.v05()
    }

    /// D-112: a project's `raw/` is a record layer — content-immutable at the
    /// gate, its references historical. Before: pages like any other.
    pub fn project_records(self) -> bool {
        self.v05()
    }

    /// R-210, R-211 (D-115): the tree's declared uncertainty markers and
    /// history headings are reported on permanent and reference pages. Before:
    /// the two keys were unknown (R-161).
    pub fn declared_markers(self) -> bool {
        self.v05()
    }

    /// D-122: a page's date is its last content change in history; nothing
    /// writes `updated:`, and a page that carries it is reported. Before:
    /// `updated:` required, bumped by the tooling, checked against history.
    pub fn derived_dates(self) -> bool {
        self.v05()
    }

    /// D-123: a router line may route a directory, and every page under it
    /// is reachable. Before: a router line routes one page.
    pub fn directory_routes(self) -> bool {
        self.v05()
    }

    /// D-124: debt and questions are directories of one-item files, and a
    /// closed item leaves with its file. Before: one ledger file per list,
    /// closed items included.
    pub fn item_files(self) -> bool {
        self.v05()
    }

    /// D-125: the journal is version-control history — a commit that changes
    /// the docs or carries `Docs:` is an entry, `docsys journal` renders it,
    /// and the `commit-msg` gate holds what `require` asks of a commit.
    /// Before: `work/journal.md`, rotated into slices.
    pub fn journal_from_history(self) -> bool {
        self.v05()
    }

    /// D-126: a page's verification is read from history — an approval
    /// commit (`Approved-by:` naming a maintainer) after its last body change;
    /// nothing is written into the page. Before: the record in the page's
    /// frontmatter (D-101's anchored record on a 0.5 tree from before).
    pub fn verification_from_history(self) -> bool {
        self.v05()
    }

    /// D-127: `graduate apply --confirmed` moves the last blocks and removes
    /// the work file; the commit names the destinations. Before: the file
    /// stays as `graduated`, its `confirmed:` written on it.
    pub fn graduation_removes(self) -> bool {
        self.v05()
    }

    /// D-116: `lint`, `refs` and `gate` end with a pointer to `docsys feedback`
    /// under a finding of a rule that reads free text. Before: the findings alone.
    pub fn finding_pointers(self) -> bool {
        self.v05()
    }

    /// D-040: a call that runs `git add` before its commit is asked about the
    /// untracked files that `git add` may take too. Before: the tracked
    /// changes alone.
    pub fn untracked_in_question(self) -> bool {
        self.v05()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn the_declared_minor_decides_and_anything_unreadable_is_0_4() {
        assert_eq!(Era::of_spec(Some(" docsys/0.5")), Era(5));
        assert_eq!(Era::of_spec(Some("\"docsys/0.12\"")), Era(12));
        assert_eq!(Era::of_spec(Some("docsys/0.4")), Era(4));
        assert_eq!(Era::of_spec(None), Era(4));
        assert_eq!(Era::of_spec(Some("docsys/1.0")), Era(4));
        assert!(Era(5).anchored_verification() && !Era(4).anchored_verification());
    }
}
