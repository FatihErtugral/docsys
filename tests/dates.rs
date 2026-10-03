#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! A page's date is history's on a docsys/0.5 tree (R-050, D-122): no command
//! and no relay writes `updated:`, and no text an agent reads names it.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-dates-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(ok.status.success(), "git {args:?}: {ok:?}");
}

fn docsys(dir: &Path, args: &[&str], stdin: Option<&str>) -> String {
    // the git hooks a command's own commit runs find this build first
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_docsys"));
    let path = format!(
        "{}:{}",
        bin.parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = Command::new(&bin)
        .args(args)
        .current_dir(dir)
        .env("PATH", path)
        .env_remove("DOCSYS_DISPATCHED")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.unwrap_or("").as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "docsys {args:?}: {text}");
    text
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// Every line in `dir` that writes or names the date field.
fn mentions(dir: &Path) -> Vec<String> {
    let mut all = Vec::new();
    files(dir, &mut all);
    let mut hits = Vec::new();
    for f in all {
        let Ok(text) = fs::read_to_string(&f) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            if line.contains("updated:") || line.contains("`updated`") {
                hits.push(format!("{}:{}: {line}", f.display(), i + 1));
            }
        }
    }
    hits
}

const CODE: &str = "pub fn f(x: u32) -> u32 {\n    x + 1\n}\n";

#[test]
fn no_command_and_no_relay_writes_a_date_and_no_agent_text_names_it() {
    let repo = tmp("writers");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/app.rs"), CODE).unwrap();
    docsys(&repo, &["adopt"], None);
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &["-c", "core.hooksPath=/dev/null", "commit", "-qm", "adopt"],
    );

    // a permanent page and a work page, written as an agent would
    docsys(
        &repo,
        &["page", "new", "reference", "a", "--title", "A"],
        None,
    );
    docsys(
        &repo,
        &["page", "new", "feature", "f", "--title", "F"],
        None,
    );
    let page = repo.join("docs/reference/a.md");
    let body = fs::read_to_string(&page).unwrap();
    let body = body
        .split("<!-- opening")
        .next()
        .unwrap()
        .trim_end()
        .to_string();
    // a line a branch from before the upgrade still carries: no writer may move it
    let body = body.replacen(
        "type: reference\n",
        "type: reference\nupdated: 2020-01-01\n",
        1,
    );
    fs::write(
        &page,
        format!("{body}\n\nThis page states what f does; read it first.\n"),
    )
    .unwrap();
    let feature = repo.join("docs/work/features/f.md");
    let text = fs::read_to_string(&feature).unwrap();
    fs::write(
        &feature,
        text.replace(
            "## Contract surface\n",
            "## Contract surface\n\nf adds one.\n",
        ),
    )
    .unwrap();

    // the post-edit relay after an agent's edit
    let payload = format!(
        "{{\"tool_name\":\"Edit\",\"tool_input\":{{\"file_path\":\"{}\"}}}}",
        page.display()
    );
    docsys(
        &repo,
        &["hook", "post-tool-use", "--root", "docs", "--stdin"],
        Some(&payload),
    );

    // a pin and a re-read after the code moved
    docsys(
        &repo,
        &["pin", "reference/a", "src/app.rs", "--symbol", "f"],
        None,
    );
    fs::write(repo.join("src/app.rs"), CODE.replace("x + 1", "x + 2")).unwrap();
    docsys(&repo, &["pin", "--refresh", "reference/a"], None);
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &["-c", "core.hooksPath=/dev/null", "commit", "-qm", "page"],
    );
    // a graduation into the page
    let plan = docsys(&repo, &["graduate", "plan", "work/features/f.md"], None);
    let filled = plan.replacen("\tkeep", "\tmove:reference/a", 1);
    let plan_file = repo.with_extension("plan.tsv");
    fs::write(&plan_file, filled).unwrap();
    docsys(
        &repo,
        &["graduate", "apply", "--plan", plan_file.to_str().unwrap()],
        None,
    );

    let written: Vec<String> = mentions(&repo.join("docs"))
        .into_iter()
        .filter(|l| !(l.contains("reference/a.md:") && l.ends_with(": updated: 2020-01-01")))
        .collect();
    assert!(
        written.is_empty(),
        "a date was written:\n{}",
        written.join("\n")
    );
    let text = fs::read_to_string(&page).unwrap();
    assert!(text.contains("\nupdated: 2020-01-01\n"), "{text}");

    // what an agent reads: the rules block, the installed layer, the routing
    let mut agent = mentions(&repo.join(".claude"));
    for f in ["AGENTS.md", "ADOPTION.md"] {
        let text = fs::read_to_string(repo.join(f)).unwrap_or_default();
        agent.extend(
            text.lines()
                .filter(|l| l.contains("updated:") || l.contains("`updated`"))
                .map(|l| format!("{f}: {l}")),
        );
    }
    for (name, text) in [
        ("routing", docsys::hook::ROUTING),
        ("kb routing", docsys::hook::KB_ROUTING),
    ] {
        agent.extend(
            text.lines()
                .filter(|l| l.contains("updated:") || l.contains("`updated`"))
                .map(|l| format!("{name}: {l}")),
        );
    }
    assert!(
        agent.is_empty(),
        "agent text names the date field:\n{}",
        agent.join("\n")
    );
}
