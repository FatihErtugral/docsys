//! A line diff, for a person to read what a regeneration would change (D-111).
//! The longest common subsequence of two texts' lines, printed in the unified
//! form: `-` for a line only the old text has, `+` for one only the new text
//! has, a space for a line both keep, and an `@@ -a,b +c,d @@` header over
//! each hunk.

/// One step from the old sequence to the new one, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    /// `old[i] == new[j]`, kept
    Keep(usize, usize),
    /// `old[i]`, removed
    Remove(usize),
    /// `new[j]`, added
    Add(usize),
}

/// The table is quadratic: past this many cells the changed middle of a pair
/// is shown as one replacement — a correct diff, only not the shortest.
const MAX_CELLS: usize = 1 << 24;

/// The steps from `old` to `new`, keeping a longest common subsequence.
pub fn edits<T: PartialEq>(old: &[T], new: &[T]) -> Vec<Edit> {
    edits_within(old, new, MAX_CELLS)
}

fn edits_within<T: PartialEq>(old: &[T], new: &[T], max_cells: usize) -> Vec<Edit> {
    let head = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let old_rest = old.get(head..).unwrap_or(&[]);
    let new_rest = new.get(head..).unwrap_or(&[]);
    let tail = old_rest
        .iter()
        .rev()
        .zip(new_rest.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let a = old_rest.get(..old_rest.len() - tail).unwrap_or(&[]);
    let b = new_rest.get(..new_rest.len() - tail).unwrap_or(&[]);
    let (n, m) = (a.len(), b.len());
    let mut out: Vec<Edit> = (0..head).map(|i| Edit::Keep(i, i)).collect();
    let width = m + 1;
    let cells = (n + 1).saturating_mul(width);
    let (mut i, mut j) = (0, 0);
    if n > 0 && m > 0 && cells <= max_cells {
        // lcs[i * width + j]: the longest common subsequence of a[i..] and b[j..]
        let mut lcs = vec![0u32; cells];
        let at = |t: &[u32], i: usize, j: usize| t.get(i * width + j).copied().unwrap_or(0);
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                let v = if a.get(i) == b.get(j) {
                    at(&lcs, i + 1, j + 1) + 1
                } else {
                    at(&lcs, i + 1, j).max(at(&lcs, i, j + 1))
                };
                if let Some(c) = lcs.get_mut(i * width + j) {
                    *c = v;
                }
            }
        }
        while i < n && j < m {
            if a.get(i) == b.get(j) {
                out.push(Edit::Keep(head + i, head + j));
                i += 1;
                j += 1;
            } else if at(&lcs, i + 1, j) >= at(&lcs, i, j + 1) {
                out.push(Edit::Remove(head + i));
                i += 1;
            } else {
                out.push(Edit::Add(head + j));
                j += 1;
            }
        }
    }
    out.extend((i..n).map(|i| Edit::Remove(head + i)));
    out.extend((j..m).map(|j| Edit::Add(head + j)));
    out.extend((0..tail).map(|k| Edit::Keep(head + n + k, head + m + k)));
    out
}

/// `old` to `new` as a unified diff with `context` lines around each change;
/// empty when both texts hold the same lines.
pub fn unified(old: &str, new: &str, old_name: &str, new_name: &str, context: usize) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let steps = edits(&a, &b);
    // the old and new line counts consumed before each step
    let mut before = Vec::with_capacity(steps.len() + 1);
    let (mut i, mut j) = (0usize, 0usize);
    for s in &steps {
        before.push((i, j));
        match s {
            Edit::Keep(..) => {
                i += 1;
                j += 1;
            }
            Edit::Remove(_) => i += 1,
            Edit::Add(_) => j += 1,
        }
    }
    before.push((i, j));
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for (k, s) in steps.iter().enumerate() {
        if matches!(s, Edit::Keep(..)) {
            continue;
        }
        let (start, end) = (
            k.saturating_sub(context),
            (k + 1 + context).min(steps.len()),
        );
        match hunks.last_mut() {
            Some((_, e)) if start <= *e => *e = end,
            _ => hunks.push((start, end)),
        }
    }
    if hunks.is_empty() {
        return String::new();
    }
    // GNU's form: a one-line range has no count, an empty one names the line before it
    let range = |first: usize, count: usize| match count {
        0 => format!("{first},0"),
        1 => format!("{}", first + 1),
        _ => format!("{},{count}", first + 1),
    };
    let mut out = format!("--- {old_name}\n+++ {new_name}\n");
    for (start, end) in hunks {
        let (Some(&(i0, j0)), Some(&(i1, j1))) = (before.get(start), before.get(end)) else {
            continue;
        };
        out.push_str(&format!(
            "@@ -{} +{} @@\n",
            range(i0, i1 - i0),
            range(j0, j1 - j0)
        ));
        for s in steps.get(start..end).unwrap_or(&[]) {
            let (mark, line) = match *s {
                Edit::Keep(i, _) => (' ', a.get(i)),
                Edit::Remove(i) => ('-', a.get(i)),
                Edit::Add(j) => ('+', b.get(j)),
            };
            out.push(mark);
            out.push_str(line.copied().unwrap_or(""));
            out.push('\n');
        }
    }
    out
}

/// A template change that touches lines its owner changed too: the owner's
/// lines, which the merge keeps, and the template's, which it does not apply.
#[derive(Debug, PartialEq, Eq)]
pub struct Clash {
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
}

/// Three texts merged by their lines: what changed from `base` to `theirs`
/// (a template between two versions) applied to `ours` (the owner's file).
/// Where both changed the same lines, `ours` stays and the template's
/// change is returned apart — nothing the owner wrote is dropped.
pub fn merge3(base: &str, ours: &str, theirs: &str) -> (String, Vec<Clash>) {
    let b: Vec<&str> = base.lines().collect();
    let o: Vec<&str> = ours.lines().collect();
    let t: Vec<&str> = theirs.lines().collect();
    let kept = |other: &[&str]| {
        let mut at = vec![None; b.len()];
        for e in edits(&b, other) {
            if let Edit::Keep(i, j) = e {
                if let Some(slot) = at.get_mut(i) {
                    *slot = Some(j);
                }
            }
        }
        at
    };
    let (in_o, in_t) = (kept(&o), kept(&t));
    let mut stable: Vec<(usize, usize, usize)> = (0..b.len())
        .filter_map(|i| Some((i, (*in_o.get(i)?)?, (*in_t.get(i)?)?)))
        .collect();
    stable.push((b.len(), o.len(), t.len()));
    fn chunk<'a>(v: &[&'a str], from: usize, to: usize) -> Vec<&'a str> {
        v.get(from..to).unwrap_or(&[]).to_vec()
    }
    let mut out: Vec<&str> = Vec::new();
    let mut clashes = Vec::new();
    let (mut bi, mut oi, mut ti) = (0, 0, 0);
    for (bs, os, ts) in stable {
        let (cb, co, ct) = (chunk(&b, bi, bs), chunk(&o, oi, os), chunk(&t, ti, ts));
        if co == cb {
            out.extend(ct);
        } else if ct == cb || co == ct {
            out.extend(co);
        } else {
            clashes.push(Clash {
                ours: co.iter().map(|l| l.to_string()).collect(),
                theirs: ct.iter().map(|l| l.to_string()).collect(),
            });
            out.extend(co);
        }
        if let Some(line) = b.get(bs) {
            out.push(line);
        }
        (bi, oi, ti) = (bs + 1, os + 1, ts + 1);
    }
    let mut text = out.join("\n");
    if !text.is_empty() && (ours.ends_with('\n') || ours.is_empty()) {
        text.push('\n');
    }
    (text, clashes)
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

    /// The edits replay: every old index once, in order, every new index
    /// once, in order, and a kept pair holds equal values.
    fn replays<T: PartialEq + std::fmt::Debug>(old: &[T], new: &[T], steps: &[Edit]) {
        let (mut i, mut j) = (0, 0);
        for s in steps {
            match *s {
                Edit::Keep(a, b) => {
                    assert_eq!((a, b), (i, j), "{steps:?}");
                    assert_eq!(old[a], new[b]);
                    i += 1;
                    j += 1;
                }
                Edit::Remove(a) => {
                    assert_eq!(a, i, "{steps:?}");
                    i += 1;
                }
                Edit::Add(b) => {
                    assert_eq!(b, j, "{steps:?}");
                    j += 1;
                }
            }
        }
        assert_eq!((i, j), (old.len(), new.len()), "{steps:?}");
    }

    fn kept(steps: &[Edit]) -> usize {
        steps.iter().filter(|s| matches!(s, Edit::Keep(..))).count()
    }

    #[test]
    fn the_steps_keep_a_longest_common_subsequence() {
        let a: Vec<char> = "ABCABBA".chars().collect();
        let b: Vec<char> = "CBABAC".chars().collect();
        let steps = edits(&a, &b);
        replays(&a, &b, &steps);
        assert_eq!(kept(&steps), 4, "{steps:?}");
        let same = edits(&a, &a);
        replays(&a, &a, &same);
        assert_eq!(kept(&same), a.len());
        let none: Vec<char> = Vec::new();
        replays(&a, &none, &edits(&a, &none));
        replays(&none, &b, &edits(&none, &b));
    }

    #[test]
    fn a_pair_too_large_for_the_table_is_one_replacement() {
        let a = ["k", "x", "y", "k"];
        let b = ["k", "y", "z", "k"];
        let steps = edits_within(&a, &b, 1);
        replays(&a, &b, &steps);
        assert_eq!(
            steps,
            vec![
                Edit::Keep(0, 0),
                Edit::Remove(1),
                Edit::Remove(2),
                Edit::Add(1),
                Edit::Add(2),
                Edit::Keep(3, 3)
            ]
        );
        assert_eq!(kept(&edits(&a, &b)), 3);
    }

    #[test]
    fn equal_texts_have_no_diff() {
        assert_eq!(unified("a\nb\n", "a\nb\n", "old", "new", 3), "");
        assert_eq!(unified("", "", "old", "new", 3), "");
    }

    #[test]
    fn a_change_is_shown_with_its_context_under_a_hunk_header() {
        let old = "a\nb\nc\nd\ne\n";
        let new = "a\nb\nX\nd\ne\n";
        assert_eq!(
            unified(old, new, "old.yml", "new.yml", 1),
            "--- old.yml\n+++ new.yml\n@@ -2,3 +2,3 @@\n b\n-c\n+X\n d\n"
        );
        assert_eq!(
            unified("", "x\n", "a", "b", 3),
            "--- a\n+++ b\n@@ -0,0 +1 @@\n+x\n"
        );
        assert_eq!(
            unified("x\ny\n", "y\n", "a", "b", 0),
            "--- a\n+++ b\n@@ -1 +0,0 @@\n-x\n"
        );
    }

    #[test]
    fn changes_far_apart_are_two_hunks_and_near_ones_one() {
        let lines = |swap: &[(u32, &str)]| -> String {
            (1..=12)
                .map(|n| match swap.iter().find(|(k, _)| *k == n) {
                    Some((_, s)) => format!("{s}\n"),
                    None => format!("{n}\n"),
                })
                .collect()
        };
        let old = lines(&[]);
        let far = lines(&[(2, "two"), (11, "eleven")]);
        let d = unified(&old, &far, "a", "b", 2);
        assert_eq!(d.matches("@@ ").count(), 2, "{d}");
        assert!(d.contains("@@ -1,4 +1,4 @@\n 1\n-2\n+two\n 3\n 4\n"), "{d}");
        assert!(
            d.contains("@@ -9,4 +9,4 @@\n 9\n 10\n-11\n+eleven\n 12\n"),
            "{d}"
        );
        let near = lines(&[(5, "five"), (9, "nine")]);
        let d = unified(&old, &near, "a", "b", 2);
        assert_eq!(d.matches("@@ ").count(), 1, "{d}");
        assert!(d.contains("@@ -3,9 +3,9 @@\n"), "{d}");
    }

    #[test]
    fn a_three_way_merge_keeps_the_owners_lines_and_applies_the_template() {
        let base = "# T\nname: (unset)\n## Rules\nrule one\nrule two\nend\n";
        let ours = "# T\nname: Ada\n## Rules\nrule one\nrule two\nmy own line\nend\n";
        let theirs = "# T\nname: (unset)\n## Rules\nrule one, sharper\nrule two\nend\n";
        let (text, clashes) = super::merge3(base, ours, theirs);
        assert_eq!(
            text,
            "# T\nname: Ada\n## Rules\nrule one, sharper\nrule two\nmy own line\nend\n"
        );
        assert!(clashes.is_empty(), "{clashes:?}");
        // both changed the same line: the owner's stays, the template's is apart
        let ours = "# T\nname: (unset)\n## Rules\nrule one, mine\nrule two\nend\n";
        let (text, clashes) = super::merge3(base, ours, theirs);
        assert_eq!(
            text,
            "# T\nname: (unset)\n## Rules\nrule one, mine\nrule two\nend\n"
        );
        assert_eq!(
            clashes,
            vec![super::Clash {
                ours: vec!["rule one, mine".to_string()],
                theirs: vec!["rule one, sharper".to_string()],
            }]
        );
    }
}
