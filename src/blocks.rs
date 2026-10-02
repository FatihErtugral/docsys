//! Verification blocks (R-028, R-212, D-103). A verification vouches
//! for a whole page; recording the body's blocks lets a changed page say how
//! much of it still reads as verified, and lets a re-verification read only
//! the change. A block is cut by ASCII markup alone, with no language
//! knowledge: a fence, an ATX heading line, a top-level list item with what is
//! indented under it, an HTML comment, or a paragraph — a table and a block
//! quote are paragraphs. The sequence of block hashes is the verification's
//! record of the body (R-028): the body as verified is the same sequence.

use crate::fm::{Frontmatter, Value};

/// One block of a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// first and last line, 1-based, in the text that was split
    pub first: usize,
    pub last: usize,
    pub text: String,
    /// D-103's short hash
    pub hash: String,
}

/// Columns of leading whitespace, a tab advancing to the next multiple of 4.
fn indent(line: &str) -> usize {
    let mut col = 0;
    for c in line.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col += 4 - col % 4,
            _ => break,
        }
    }
    col
}

fn lead(line: &str) -> &str {
    line.trim_start_matches([' ', '\t'])
}

/// A fence opener's character and run length.
fn fence_open(line: &str) -> Option<(char, usize)> {
    let t = lead(line);
    let c = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|x| *x == c).count();
    (n >= 3).then_some((c, n))
}

fn fence_closes(line: &str, c: char, n: usize) -> bool {
    let t = line.trim();
    !t.is_empty() && t.chars().all(|x| x == c) && t.chars().count() >= n
}

fn heading(line: &str) -> bool {
    if indent(line) > 3 {
        return false;
    }
    let t = lead(line);
    let n = t.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&n) && matches!(t.chars().nth(n), None | Some(' ' | '\t'))
}

/// The content column of a top-level list item `line` opens.
fn item(line: &str) -> Option<usize> {
    let at = indent(line);
    if at > 3 {
        return None;
    }
    let t = lead(line);
    let marker = if t.starts_with(['-', '*', '+']) {
        1
    } else {
        let digits = t.chars().take_while(char::is_ascii_digit).count();
        if !(1..=9).contains(&digits) || !matches!(t.chars().nth(digits), Some('.' | ')')) {
            return None;
        }
        digits + 1
    };
    // the marker is ASCII, so its length in chars is its length in bytes
    let rest = t.get(marker..).unwrap_or("");
    if rest.trim().is_empty() {
        return Some(at + marker + 1);
    }
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let gap = indent(rest);
    Some(at + marker + if gap > 4 { 1 } else { gap })
}

fn comment_open(line: &str) -> bool {
    indent(line) <= 3 && lead(line).starts_with("<!--")
}

/// A line that ends a paragraph or a lazily continued item by opening a block.
fn interrupts(line: &str) -> bool {
    heading(line)
        || item(line).is_some()
        || (indent(line) <= 3 && fence_open(line).is_some())
        || comment_open(line)
}

/// Lines that belong to the block whatever their indentation, until closed.
#[derive(Clone, Copy)]
enum Open {
    Fence(char, usize),
    Comment,
}

impl Open {
    fn closes(self, line: &str) -> bool {
        match self {
            Open::Fence(c, n) => fence_closes(line, c, n),
            Open::Comment => line.contains("-->"),
        }
    }

    /// What a line inside a block opens, if anything.
    fn at(line: &str) -> Option<Open> {
        if let Some((c, n)) = fence_open(line) {
            return Some(Open::Fence(c, n));
        }
        let rest = lead(line).strip_prefix("<!--")?;
        (!rest.contains("-->")).then_some(Open::Comment)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Paragraph,
    /// a list item and its content column
    Item(usize),
    /// a fence or a comment: it ends where it closes
    Verbatim,
}

struct Cutter<'a> {
    out: Vec<Block>,
    shape: Option<Shape>,
    first: usize,
    lines: Vec<&'a str>,
    /// blank lines inside an item, kept only if the item goes on
    blanks: Vec<&'a str>,
}

impl<'a> Cutter<'a> {
    fn begin(&mut self, shape: Shape, n: usize, line: &'a str) {
        self.shape = Some(shape);
        self.first = n;
        self.lines.push(line);
    }

    fn push(&mut self, line: &'a str) {
        self.lines.append(&mut self.blanks);
        self.lines.push(line);
    }

    fn flush(&mut self) {
        self.blanks.clear();
        self.shape = None;
        if self.lines.is_empty() {
            return;
        }
        let text = self.lines.join("\n");
        self.out.push(Block {
            first: self.first,
            last: self.first + self.lines.len() - 1,
            hash: short_hash(&text),
            text,
        });
        self.lines.clear();
    }
}

/// The body cut into blocks (D-103).
pub fn split(body: &str) -> Vec<Block> {
    let mut c = Cutter {
        out: Vec::new(),
        shape: None,
        first: 0,
        lines: Vec::new(),
        blanks: Vec::new(),
    };
    let mut open: Option<Open> = None;
    for (i, line) in body.lines().enumerate() {
        let n = i + 1;
        if let Some(o) = open {
            c.push(line);
            if o.closes(line) {
                open = None;
                if c.shape == Some(Shape::Verbatim) {
                    c.flush();
                }
            }
            continue;
        }
        let blank = line.trim().is_empty();
        match c.shape {
            Some(Shape::Item(col)) => {
                if blank {
                    c.blanks.push(line);
                    continue;
                }
                if indent(line) >= col || (c.blanks.is_empty() && !interrupts(line)) {
                    c.push(line);
                    open = Open::at(line);
                    continue;
                }
                c.flush();
            }
            Some(Shape::Paragraph) => {
                if blank {
                    c.flush();
                    continue;
                }
                if !interrupts(line) {
                    c.push(line);
                    continue;
                }
                c.flush();
            }
            Some(Shape::Verbatim) => c.flush(),
            None if blank => continue,
            None => {}
        }
        // a block opens at this line
        if let Some((ch, len)) = fence_open(line).filter(|_| indent(line) <= 3) {
            c.begin(Shape::Verbatim, n, line);
            open = Some(Open::Fence(ch, len));
        } else if comment_open(line) {
            c.begin(Shape::Verbatim, n, line);
            open = Open::at(line);
            if open.is_none() {
                c.flush();
            }
        } else if heading(line) {
            c.begin(Shape::Verbatim, n, line);
            c.flush();
        } else if let Some(col) = item(line) {
            c.begin(Shape::Item(col), n, line);
        } else {
            c.begin(Shape::Paragraph, n, line);
        }
    }
    c.flush();
    c.out
}

/// The first 12 hex digits of the SHA-256 of a block's canonical form (R-113).
pub fn short_hash(text: &str) -> String {
    crate::fresh::sha256_hex(crate::fresh::canonical(text).as_bytes())
        .chars()
        .take(12)
        .collect()
}

/// The body's block hashes, in order — what `verified_blocks` records.
pub fn hashes(body: &str) -> Vec<String> {
    split(body).into_iter().map(|b| b.hash).collect()
}

/// What became of a block between the record and the body now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// a recorded block found again, in order
    Same,
    /// in the place of a recorded block that is not found again
    Changed,
    New,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Compare {
    /// one per current block
    pub current: Vec<Fate>,
    /// the recorded blocks no current block stands in for, by index
    pub removed: Vec<usize>,
}

/// The record against the body now: the longest common subsequence of the two
/// hash sequences is what is found again. Between two blocks found again, the
/// current ones pair with the recorded ones in order as changed; the rest are
/// new, or removed.
pub fn compare(recorded: &[String], current: &[String]) -> Compare {
    let mut out = Compare {
        current: vec![Fate::New; current.len()],
        removed: Vec::new(),
    };
    let mut gone: Vec<usize> = Vec::new();
    let mut added: Vec<usize> = Vec::new();
    let settle = |gone: &mut Vec<usize>, added: &mut Vec<usize>, out: &mut Compare| {
        for (k, j) in added.iter().enumerate() {
            if k < gone.len() {
                if let Some(f) = out.current.get_mut(*j) {
                    *f = Fate::Changed;
                }
            }
        }
        out.removed.extend(gone.iter().skip(added.len()));
        gone.clear();
        added.clear();
    };
    for step in crate::diff::edits(recorded, current) {
        match step {
            crate::diff::Edit::Keep(_, j) => {
                settle(&mut gone, &mut added, &mut out);
                if let Some(f) = out.current.get_mut(j) {
                    *f = Fate::Same;
                }
            }
            crate::diff::Edit::Remove(i) => gone.push(i),
            crate::diff::Edit::Add(j) => added.push(j),
        }
    }
    settle(&mut gone, &mut added, &mut out);
    out
}

/// A page's block record, `verified_blocks`.
pub fn record_of(fm: &Frontmatter) -> Option<Vec<String>> {
    fm.fields
        .get("verified_blocks")
        .and_then(Value::as_list)
        .map(|l| l.iter().map(|h| h.trim().to_string()).collect())
}

/// Whether the body still reads as recorded (R-024): the same blocks, in the
/// same order. `None` without a record.
pub fn holds(fm: &Frontmatter, text: &str) -> Option<bool> {
    record_of(fm).map(|r| r == hashes(&crate::fresh::body_text(text)))
}

/// How much of a page still reads as verified (R-028, R-212).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// the recorded blocks found again, less those a stale bound pin backs
    pub found: usize,
    /// the body's blocks now
    pub of: usize,
    /// the body is not the one verified
    pub moved: bool,
}

impl Reading {
    /// Worth saying: the body moved, or a block stopped reading as verified.
    pub fn partial(&self) -> bool {
        self.moved || self.found < self.of
    }
}

/// The reading of a page text against its block record; `stale` holds the
/// blocks whose bound pin is stale. `None` without a record.
pub fn reading(fm: &Frontmatter, text: &str, stale: &[String]) -> Option<Reading> {
    let recorded = record_of(fm)?;
    let body = crate::fresh::body_text(text);
    let current = hashes(&body);
    let fates = compare(&recorded, &current).current;
    let found = fates
        .iter()
        .zip(&current)
        .filter(|(f, h)| **f == Fate::Same && !stale.contains(h))
        .count();
    let moved = recorded != current;
    Some(Reading {
        found,
        of: current.len(),
        moved,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    /// Each block as its text, `|` between lines.
    fn texts(body: &str) -> Vec<String> {
        split(body)
            .iter()
            .map(|b| b.text.replace('\n', "|"))
            .collect()
    }

    #[test]
    fn one_row_per_markup_rule() {
        let rows: [(&str, &str, &[&str]); 9] = [
            (
                "a blank line ends a paragraph",
                "one\ntwo\n\nthree\n",
                &["one|two", "three"],
            ),
            (
                "a fence is one block, a blank line inside it included",
                "```sh\nmake\n\nmake test\n```\nafter\n",
                &["```sh|make||make test|```", "after"],
            ),
            (
                "a fence closes only on a run of its own character, as long",
                "~~~~\n```\n~~~\n\n~~~~\n",
                &["~~~~|```|~~~||~~~~"],
            ),
            (
                "a list item holds its nested list and a lazy continuation",
                "- one\n  - nested\n    deeper\nlazy line\n\n  more of one\n- two\n",
                &[
                    "- one|  - nested|    deeper|lazy line||  more of one",
                    "- two",
                ],
            ),
            (
                "an unindented line after a blank one ends the item",
                "1. first\n\nafter the list\n2) second\n",
                &["1. first", "after the list", "2) second"],
            ),
            (
                "a heading glued to a paragraph is a block of its own",
                "# Title\nText under it.\n## Next\n",
                &["# Title", "Text under it.", "## Next"],
            ),
            (
                "an HTML comment is one block, through -->",
                "<!-- a note\n\nstill the note -->\nText.\n",
                &["<!-- a note||still the note -->", "Text."],
            ),
            (
                "a table and a block quote are paragraphs",
                "| a | b |\n|---|---|\n| 1 | 2 |\n\n> quoted\n> more\n",
                &["| a | b |||---|---||| 1 | 2 |", "> quoted|> more"],
            ),
            (
                "a list item opens a block after a paragraph line",
                "Intro:\n- a\n* b\n+ c\n    #notheading\n",
                &["Intro:", "- a", "* b", "+ c|    #notheading"],
            ),
        ];
        for (rule, body, want) in rows {
            assert_eq!(texts(body), want, "{rule}");
        }
    }

    #[test]
    fn lines_are_numbered_and_an_open_fence_runs_to_the_end() {
        let b = split("\n# T\n\n```\ncode\n\nmore\n");
        let at: Vec<(usize, usize)> = b.iter().map(|b| (b.first, b.last)).collect();
        assert_eq!(at, vec![(2, 2), (4, 7)]);
    }

    #[test]
    fn a_hash_is_twelve_hex_of_the_canonical_form() {
        let lf = hashes("Para one.\n\n- item\n  more\n");
        let crlf = hashes("Para one.  \r\n\r\n- item\r\n  more\r\n");
        assert_eq!(lf, crlf);
        assert_eq!(lf.len(), 2);
        assert!(lf
            .iter()
            .all(|h| h.len() == 12 && h.chars().all(|c| c.is_ascii_hexdigit())));
        // sha256("# Edited\n"), first 12 hex digits
        assert_eq!(hashes("# Edited\n"), vec!["941ba81fbfec".to_string()]);
    }

    #[test]
    fn the_record_is_compared_in_order() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let c = compare(&s(&["a", "b", "c"]), &s(&["a", "b2", "c", "d"]));
        assert_eq!(
            c.current,
            vec![Fate::Same, Fate::Changed, Fate::Same, Fate::New]
        );
        assert!(c.removed.is_empty());
        let c = compare(&s(&["a", "b", "c"]), &s(&["a", "c"]));
        assert_eq!(c.current, vec![Fate::Same, Fate::Same]);
        assert_eq!(c.removed, vec![1]);
        // a block moved past another is found again once: in order
        let c = compare(&s(&["a", "b", "c"]), &s(&["c", "a", "b"]));
        assert_eq!(c.current.iter().filter(|f| **f == Fate::Same).count(), 2);
    }

    #[test]
    fn the_record_is_an_inline_list_even_when_reflowed() {
        let fm = crate::fm::parse(
            "---\nid: x\nverified_blocks: [941ba81fbfec,\n  28949667d156]\n---\nBody.\n",
        )
        .unwrap();
        assert_eq!(
            record_of(&fm),
            Some(vec!["941ba81fbfec".to_string(), "28949667d156".to_string()])
        );
    }

    #[test]
    fn an_empty_body_records_an_empty_list_and_still_holds() {
        let page = "---\nid: x\nverified_blocks: []\n---\n";
        let fm = crate::fm::parse(page).unwrap();
        assert_eq!(record_of(&fm), Some(Vec::new()));
        assert_eq!(holds(&fm, page), Some(true));
        assert_eq!(holds(&fm, &format!("{page}New text.\n")), Some(false));
    }
}
