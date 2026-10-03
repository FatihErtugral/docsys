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
        "consume", "inbox",
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
    // an unknown flag: that command's help, not the whole list
    for args in [&["lint", "--nope"][..], &["debt", "add", "--nope"]] {
        let out = Command::new(bin())
            .args(args)
            .env("DOCSYS_NO_AUTO_INSTALL", "1")
            .current_dir(std::env::temp_dir())
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stderr).into_owned();
        let name = args
            .split_last()
            .map(|(_, n)| n.join(" "))
            .unwrap_or_default();
        assert_eq!(out.status.code(), Some(2), "{args:?}: {text}");
        assert!(text.contains("`--nope`"), "{args:?}: {text}");
        assert!(
            text.contains(&format!("docsys {name} ")),
            "{args:?}: {text}"
        );
        assert!(!text.contains("Commands:"), "{args:?}: {text}");
    }
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

/// The texts an agent and a person read — the rules block, the skills, the
/// commands `adopt` writes, the first-turn routing with the tree's digest
/// under `commit_policy: require`, help and the feedback guide — with their
/// line breaks folded.
fn homes() -> Vec<(&'static str, String)> {
    let dir = std::env::temp_dir().join(format!("docsys-help-homes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        assert!(Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap()
            .status
            .success());
    };
    git(&["init", "-q"]);
    let docsys = |args: &[&str], input: &str| {
        let mut child = Command::new(bin())
            .args(args)
            .current_dir(&dir)
            .env("DOCSYS_NO_AUTO_INSTALL", "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        std::io::Write::write_all(child.stdin.as_mut().unwrap(), input.as_bytes()).unwrap();
        String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout).into_owned()
    };
    docsys(&["adopt"], "");
    let meta = dir.join("docs/.docmeta.yml");
    let text = std::fs::read_to_string(&meta).unwrap();
    std::fs::write(
        &meta,
        text.replace("commit_policy: ask", "commit_policy: require"),
    )
    .unwrap();
    let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).unwrap();
    let export = read(".claude/skills/docsys-export/SKILL.md");
    let commands: String = ["interview", "seed", "sync", "upgrade"]
        .iter()
        .map(|c| read(&format!(".claude/commands/docsys-{c}.md")))
        .collect();
    let feedback = docsys(&["feedback"], "");
    let routing = docsys(
        &["hook", "user-prompt-submit", "--root", "docs"],
        &format!(
            r#"{{"session_id":"homes-{}","prompt":"hi"}}"#,
            std::process::id()
        ),
    );
    let block = docsys(&["rules", "--agents-md"], "");
    let _ = std::fs::remove_dir_all(&dir);
    // the overview holds every purpose; each command adds its flags
    let mut help = run(&["--help"]).1;
    for c in docsys::help::COMMANDS {
        for (_, what) in c.flags {
            help.push_str(what);
            help.push('\n');
        }
    }
    let fold = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
    vec![
        ("block", fold(&block)),
        ("skill", fold(docsys::agents::skill_text())),
        ("routing", fold(&routing)),
        ("help", fold(&help)),
        ("export", fold(&export)),
        ("commands", fold(&commands)),
        ("feedback", fold(&feedback)),
    ]
}

/// Each fact has one home (D-114, D-129). A fact said again in other words
/// shares no six words, so each is caught by its own words: one of them in
/// two texts, or twice in one, is a second home.
#[test]
fn each_fact_is_said_in_one_place() {
    let facts: &[(&str, &[&str])] = &[
        (
            "a commit message says why",
            &[
                "what and why",
                "message that says why",
                "record WHY",
                "commit that says why",
            ],
        ),
        ("the procedures are a command", &["rules --procedures"]),
        ("lookup comes first", &["docsys lookup <words>"]),
        ("the work types", &["feature, bug"]),
        (
            "the mechanics are the binary's",
            &["re-derive", "re-implement"],
        ),
        ("the two checks", &["refs --repo ."]),
        (
            "index.md routes the pages",
            &["index.md` routes", "index.md routes", "reachable from"],
        ),
        ("verify records an approval", &["docsys verify <page>"]),
        (
            "a pinned tree runs its version",
            &["installed on first use", "runs its own version"],
        ),
        (
            "what a re-verification reads",
            &["re-verification reads", "what to read again"],
        ),
        (
            "a compiled skill is the page",
            &["byte for byte, pinned", "pinned to the page"],
        ),
        (
            "the approval job's line",
            &["approval job adds", "approval adds"],
        ),
        (
            "the work file leaves on the word",
            &[
                "file is removed",
                "work file leaves",
                "removes the work file",
            ],
        ),
        (
            "a command's flags",
            &["flags and an example", "gives each one's flags"],
        ),
        ("require holds the turn", &["end of a turn holds"]),
        (
            "require wants the why",
            &["lands without its documentation", "needs `Docs: <why>`"],
        ),
        ("an interview's answers land verbatim", &["land verbatim"]),
        (
            "graduation on the person's word",
            &[
                "explicit human confirmation",
                "human's explicit word",
                "word that the file graduates",
            ],
        ),
        (
            "R-093's question",
            &[
                "exist anywhere else",
                "exist nowhere else",
                "exists nowhere permanent",
            ],
        ),
        ("a howto's complete steps", &["steps are complete"]),
        ("the plan skeleton", &["plan skeleton"]),
        ("blocks move as written", &["never retype", "byte for byte"]),
        ("no guess", &["never a guess", "confident guess"]),
        (
            "what the code cannot say",
            &["facts the code cannot state", "what the code cannot say"],
        ),
        (
            "contract changes update their pages",
            &["Contract-surface changes", "touching a public surface"],
        ),
        ("the work types", &["feature, bug", "feature | bug"]),
        (
            "a stale pin is read first",
            &["refreshed blind", "re-reading the page", "did not read"],
        ),
        (
            "the session that wrote it never verifies",
            &["another session"],
        ),
        (
            "memory is no source",
            &["never a source", "the note is not"],
        ),
        ("names keep their form", &["original form"]),
        ("docsys in your way", &["wrong or in your way"]),
        ("filing publishes", &["filing publishes"]),
        (
            "warnings never block",
            &["warnings accumulate", "never block"],
        ),
        ("deferred work", &["deferred on purpose"]),
        ("a repaid debt", &["once repaid", "is repaid"]),
        ("the unknown", &["not known"]),
        ("the pin is the binding", &["whole binding"]),
        (
            "what points at a file",
            &["what points at it", "names the pages that describe"],
        ),
        ("lint's exit", &["exit 1 on an error", "blocking findings"]),
        ("adopt does it all", &["sets it up", "`adopt` does both"]),
        (
            "one feature's evidence",
            &["one feature's evidence", "one feature's history"],
        ),
        (
            "a record lands once",
            &["raw/inbox/ once", "the same item lands once"],
        ),
        (
            "the plan before the move",
            &["plan first", "only the plan is printed"],
        ),
    ];
    let homes = homes();
    let mut twice = Vec::new();
    for (fact, words) in facts {
        let said: Vec<&str> = homes
            .iter()
            .flat_map(|(home, text)| {
                words
                    .iter()
                    .flat_map(move |w| std::iter::repeat_n(*home, text.matches(w).count()))
            })
            .collect();
        if said.len() != 1 {
            twice.push(format!("{fact}: {said:?}"));
        }
    }
    assert!(twice.is_empty(), "{twice:#?}");
}

/// What docsys does not know is named, and is a bad invocation (exit 2): an
/// unknown command, `help` of one, an unknown sub-command; and a flag's value
/// is never another flag (D-129).
#[test]
fn an_unknown_command_is_named_and_a_flag_is_never_a_value() {
    let dir = std::env::temp_dir().join(format!("docsys-help-unknown-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run_in = |args: &[&str]| {
        let out = Command::new(bin())
            .args(args)
            .env("DOCSYS_NO_AUTO_INSTALL", "1")
            .current_dir(&dir)
            .output()
            .unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr),
        )
    };
    for (args, named) in [
        (&["nope"][..], "`nope`"),
        (&["help", "nope"], "`nope`"),
        (&["help", "debt", "nope"], "`debt nope`"),
        (&["consume", "nope"], "`consume nope`"),
        (&["--nope", "lint"], "`--nope`"),
    ] {
        let (code, text) = run_in(args);
        assert_eq!(code, Some(2), "{args:?}: {text}");
        assert!(text.contains(named), "{args:?}: {text}");
    }
    // a value that looks like a flag is refused, never taken as a file name
    let (code, text) = run_in(&["rules", "--agents-md", "--write", "--root", "."]);
    assert_eq!(code, Some(2), "{text}");
    assert!(text.contains("`--root`"), "{text}");
    assert!(!dir.join("--root").exists(), "{text}");
    // `--plan` is no flag of rules: `--write` is
    let (code, text) = run_in(&["rules", "--agents-md", "--plan", "x.md"]);
    assert_eq!(code, Some(2), "{text}");
    assert!(text.contains("docsys rules "), "{text}");
    assert!(!dir.join("x.md").exists(), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A reader that closed its end — `| head` — ends the command: docsys stops
/// writing and exits with a closed pipe's status (128 + SIGPIPE), quietly,
/// whatever the command.
#[test]
fn a_closed_stdout_ends_the_command_quietly() {
    let dir = std::env::temp_dir().join(format!("docsys-help-pipe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(Command::new(bin())
        .args(["init"])
        .current_dir(&dir)
        .output()
        .unwrap()
        .status
        .success());
    for args in [
        &["--help"][..],
        &["help", "export"],
        &["rules", "--procedures"],
        &["lint"],
        &["version"],
    ] {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        let out = Command::new(bin())
            .args(args)
            .current_dir(&dir)
            .env("DOCSYS_NO_AUTO_INSTALL", "1")
            .stdout(writer)
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.is_empty(), "{args:?}: {err}");
        assert_eq!(out.status.code(), Some(141), "{args:?}: {err}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
