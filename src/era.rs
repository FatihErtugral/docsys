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

impl Era {
    pub fn of_spec(spec: Option<&str>) -> Era {
        spec.map(|s| s.trim().trim_matches(['"', '\'']))
            .and_then(|s| s.strip_prefix("docsys/0."))
            .and_then(|m| m.parse().ok())
            .map(Era)
            .unwrap_or(Era(FIRST))
    }

    pub fn of(tree: &crate::tree::DocTree) -> Era {
        Era::of_spec(tree.docmeta_str("spec"))
    }

    /// The era of the tree at `root`, read straight from its `.docmeta.yml` —
    /// for a write that has no loaded tree.
    pub fn at(root: &Path) -> Era {
        let text = std::fs::read_to_string(root.join(".docmeta.yml")).unwrap_or_default();
        Era::of_spec(text.lines().find_map(|l| l.strip_prefix("spec:")))
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

    /// D-102: R-208's history half reads the record's own window — any commit
    /// that changed the record since the body last changed, its author or
    /// trailers. Before: the commit that introduced the exact line, its author.
    pub fn record_window(self) -> bool {
        self.v05()
    }

    /// §2.4 for R-106 and R-085: a page's last change is its last CONTENT
    /// change. Before: any commit that touched the file.
    pub fn content_history(self) -> bool {
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
