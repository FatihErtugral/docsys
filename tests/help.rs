#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! Help explains every command: `docsys --help` names each with what it is
//! for, `docsys <command> --help` gives its flags and one example, and the
//! table holds every command the binary dispatches — no more, no fewer.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn run(args: &[&str]) -> (bool, String) {
    let out = Command::new(bin())
        .args(args)
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// A dispatch arm's sub-command pattern.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
enum Sub {
    None,
    Named(String),
    Any,
}

/// Every `(<command>, <sub>)` arm of `main`'s dispatch, read from its source.
fn arms() -> BTreeSet<(String, Sub)> {
    let main = include_str!("../src/main.rs");
    let body = main
        .split_once("match (cmd, sub) {")
        .map(|(_, b)| b)
        .expect("the dispatch");
    let word = |s: &str| -> Option<(String, usize)> {
        let n = s
            .bytes()
            .take_while(|b| b.is_ascii_lowercase() || *b == b'-')
            .count();
        (n > 0).then(|| (s[..n].to_string(), n))
    };
    let mut out = BTreeSet::new();
    for (at, _) in body.match_indices("(\"") {
        let rest = &body[at + 2..];
        let Some((name, n)) = word(rest) else {
            continue;
        };
        let Some(after) = rest[n..].strip_prefix("\", ") else {
            continue;
        };
        let sub = if after.starts_with("None)") {
            Sub::None
        } else if after.starts_with("_)") {
            Sub::Any
        } else if let Some(s) = after.strip_prefix("Some(\"") {
            match word(s) {
                Some((sub, m)) if s[m..].starts_with("\"))") => Sub::Named(sub),
                _ => continue,
            }
        } else if let Some(s) = after.strip_prefix("Some(") {
            match s.split_once("))") {
                Some((ident, _)) if ident.bytes().all(|b| b.is_ascii_lowercase()) => Sub::Any,
                _ => continue,
            }
        } else {
            continue;
        };
        out.insert((name, sub));
    }
    out
}

/// The table entry an arm is documented by.
fn entry_of(name: &str, sub: &Sub) -> Option<&'static str> {
    let table = docsys::help::COMMANDS;
    let find = |n: &str| table.iter().find(|c| c.name == n).map(|c| c.name);
    match sub {
        Sub::None => find(name),
        Sub::Named(s) => find(&format!("{name} {s}")),
        // a fallback or a free argument: the command itself, or its family
        Sub::Any => find(name).or_else(|| {
            table
                .iter()
                .find(|c| c.name.starts_with(&format!("{name} ")))
                .map(|c| c.name)
        }),
    }
}

#[test]
fn the_table_holds_every_command_the_binary_dispatches_and_no_other() {
    let arms = arms();
    assert!(arms.len() > 40, "the dispatch was read: {arms:?}");
    let hidden: Vec<String> = arms
        .iter()
        .filter(|(name, _)| !name.starts_with('-'))
        .filter(|(name, sub)| entry_of(name, sub).is_none())
        .map(|(name, sub)| format!("{name} {sub:?}"))
        .collect();
    assert!(
        hidden.is_empty(),
        "commands help does not explain: {hidden:?}"
    );
    let documented: BTreeSet<&str> = arms
        .iter()
        .filter_map(|(name, sub)| entry_of(name, sub))
        .collect();
    let stale: Vec<&str> = docsys::help::COMMANDS
        .iter()
        .map(|c| c.name)
        .filter(|n| !documented.contains(n))
        .collect();
    assert!(
        stale.is_empty(),
        "help explains what nothing dispatches: {stale:?}"
    );
}

#[test]
fn every_command_says_what_it_is_for_and_shows_its_flags_and_an_example() {
    let (ok, overview) = run(&["--help"]);
    assert!(ok);
    for c in docsys::help::COMMANDS {
        assert!(!c.purpose.trim().is_empty(), "{}", c.name);
        assert!(c.example.starts_with("docsys "), "{}", c.name);
        assert!(
            overview
                .lines()
                .any(|l| l.contains(&format!("docsys {} ", c.name)) && l.ends_with(c.purpose)),
            "`docsys --help` names {} with its purpose:\n{overview}",
            c.name
        );
        let mut args: Vec<&str> = c.name.split(' ').collect();
        args.push("--help");
        let (ok, text) = run(&args);
        assert!(ok, "{args:?}");
        assert!(text.contains(c.synopsis), "{}:\n{text}", c.name);
        for (flag, what) in c.flags {
            assert!(
                text.contains(flag) && text.contains(what),
                "{} {flag}:\n{text}",
                c.name
            );
        }
        assert!(
            text.contains(&format!("Example:\n  {}\n", c.example)),
            "{}:\n{text}",
            c.name
        );
    }
    // a command named alone lists its sub-commands
    let (ok, text) = run(&["graduate", "--help"]);
    assert!(ok);
    assert!(
        text.contains("docsys graduate plan ") && text.contains("docsys graduate apply "),
        "{text}"
    );
    let (ok, text) = run(&["help", "debt", "add"]);
    assert!(ok && text.starts_with("docsys debt add "), "{text}");
}

/// A flag does what its help says, or is refused: `agents --report` lists the
/// existing layer; the procedures have one home, `rules --procedures`.
#[test]
fn agents_report_and_the_procedures_each_have_their_own_command() {
    let (ok, text) = run(&["agents", "--report"]);
    assert!(ok && text.contains("existing agent layer"), "{text}");
    let out = Command::new(bin())
        .args(["agents", "--procedures"])
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("docsys rules --procedures"),
        "{out:?}"
    );
}

/// A command group named alone lists its sub-commands, not every command;
/// a command with sub-commands lists them with its own help (D-129).
#[test]
fn a_group_alone_lists_its_sub_commands() {
    for group in [
        "debt", "question", "ledger", "graduate", "seed", "page", "export", "migrate", "raw",
    ] {
        let out = Command::new(bin())
            .arg(group)
            .env("DOCSYS_NO_AUTO_INSTALL", "1")
            .current_dir(std::env::temp_dir())
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{group}: {text}");
        assert!(
            text.contains(&format!("docsys {group} ")),
            "{group}: {text}"
        );
        assert!(
            !text.contains("Commands:"),
            "{group}: the whole list: {text}"
        );
    }
    let (ok, text) = run(&["help", "journal"]);
    assert!(ok && text.contains("docsys journal add "), "{text}");
}

/// The six-word phrases of a text, its words lowercased and stripped of
/// punctuation.
fn phrases(text: &str) -> Vec<String> {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || *c == '-')
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    words.windows(6).map(|w| w.join(" ")).collect()
}

/// Each thing is said once (D-114, D-129): `--help` repeats no phrase of its
/// own, and shares none with the always-loaded rules block — the block routes
/// an intent to a command, help explains the command.
#[test]
fn help_says_each_thing_once_and_none_the_block_says() {
    let (ok, overview) = run(&["--help"]);
    assert!(ok);
    let mut seen = std::collections::BTreeSet::new();
    let twice: Vec<String> = phrases(&overview)
        .into_iter()
        .filter(|p| !seen.insert(p.clone()))
        .collect();
    assert!(twice.is_empty(), "--help says twice: {twice:?}");
    let (ok, block) = run(&["rules", "--agents-md"]);
    assert!(ok);
    let block: BTreeSet<String> = phrases(&block).into_iter().collect();
    let mut all_help = overview.clone();
    for c in docsys::help::COMMANDS {
        all_help.push_str(c.purpose);
        all_help.push('\n');
        for (_, what) in c.flags {
            all_help.push_str(what);
            all_help.push('\n');
        }
    }
    let shared: BTreeSet<String> = phrases(&all_help)
        .into_iter()
        .filter(|p| block.contains(p))
        .collect();
    assert!(shared.is_empty(), "help and the block both say: {shared:?}");
}
