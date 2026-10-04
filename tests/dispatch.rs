#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! A tree pins the docsys it runs (D-120): every command on a pinned tree
//! runs the pinned binary from the version cache, installed once on first
//! use, and nothing loops, hangs a hook or fails silently on the way. The
//! cache's binaries here are stubs that say which version ran; `cargo` is a
//! stub that installs one, so nothing reaches the registry.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-dispatch-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn executable(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A cached version that says it ran, with its arguments and the guard.
fn stub_version(home: &Path, v: &str) {
    executable(
        &home.join("versions").join(v).join("bin/docsys"),
        &format!("#!/bin/sh\necho \"docsys {v} ran: $*\"\necho \"guard=$DOCSYS_DISPATCHED\"\n"),
    );
}

/// A `cargo` that installs a stub of the version it is asked for, and logs
/// every install.
fn stub_cargo(dir: &Path, log: &Path) {
    executable(
        &dir.join("cargo"),
        &format!(
            "#!/bin/sh\n[ \"$1\" = --version ] && {{ echo 'cargo 1.0.0'; exit 0; }}\necho \"$*\" >> '{}'\nroot=''; v=''\nwhile [ $# -gt 0 ]; do case \"$1\" in --root) root=$2; shift ;; --version) v=$2; shift ;; esac; shift; done\nmkdir -p \"$root/bin\"\nprintf '#!/bin/sh\\necho \"docsys %s ran: $*\"\\n' \"$v\" > \"$root/bin/docsys\"\nchmod +x \"$root/bin/docsys\"\n",
            log.display()
        ),
    );
}

/// A `cargo` that cannot reach the registry.
fn offline_cargo(dir: &Path) {
    executable(
        &dir.join("cargo"),
        "#!/bin/sh\n[ \"$1\" = --version ] && { echo 'cargo 1.0.0'; exit 0; }\necho 'error: failed to query the registry' >&2\nexit 101\n",
    );
}

/// PATH with `dir` first, then git and a shell — and no cargo of the machine's.
fn path_with(dir: &Path) -> String {
    let tools = dir.join("tools");
    fs::create_dir_all(&tools).unwrap();
    for tool in [
        "git", "sh", "bash", "head", "sed", "cat", "mkdir", "chmod", "printf",
    ] {
        let found = Command::new("sh")
            .args(["-c", &format!("command -v {tool}")])
            .output()
            .unwrap();
        let p = String::from_utf8_lossy(&found.stdout).trim().to_string();
        let link = tools.join(tool);
        if !p.is_empty() && !link.exists() {
            std::os::unix::fs::symlink(p, link).unwrap();
        }
    }
    format!(
        "{}:{}:{}",
        dir.display(),
        bin().parent().unwrap().display(),
        tools.display()
    )
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A repository with a tree `init` made: pinned to this build.
fn repo(name: &str) -> PathBuf {
    let r = tmp(name);
    git(&r, &["init", "-q", "-b", "main"]);
    git(&r, &["config", "user.email", "t@example.invalid"]);
    git(&r, &["config", "user.name", "t"]);
    git(&r, &["config", "commit.gpgsign", "false"]);
    let out = Command::new(bin())
        .args(["init", "--root", "docs"])
        .current_dir(&r)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        fs::read_to_string(r.join("docs/.docsys-version")).unwrap(),
        format!("{}\n", env!("CARGO_PKG_VERSION"))
    );
    git(&r, &["add", "-A"]);
    git(&r, &["commit", "-qm", "init"]);
    r
}

fn pin(r: &Path, v: &str) {
    fs::write(r.join("docs/.docsys-version"), format!("{v}\n")).unwrap();
    git(r, &["commit", "-qam", &format!("pin {v}")]);
}

struct Run {
    code: i32,
    out: String,
    err: String,
}

fn run(dir: &Path, path: &str, home: &Path, env: &[(&str, &str)], args: &[&str]) -> Run {
    let mut c = Command::new(bin());
    c.args(args)
        .current_dir(dir)
        .env("PATH", path)
        .env("DOCSYS_HOME", home)
        .env_remove("DOCSYS_DISPATCHED")
        .env_remove("DOCSYS_NO_AUTO_INSTALL")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        c.env(k, v);
    }
    // a dispatch that loops would never return: give up loudly instead
    let mut child = c.spawn().unwrap();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(60) {
            let _ = child.kill();
            panic!("docsys {args:?} did not finish");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let o: Output = child.wait_with_output().unwrap();
    Run {
        code: o.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&o.stdout).into_owned(),
        err: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

/// One machine, two trees, two pins: each command, relay and gate runs the
/// tree's own version.
#[test]
fn two_trees_with_two_pins_each_run_their_own_docsys() {
    let home = tmp("two-home");
    stub_version(&home, "7.7.7");
    let path = path_with(&tmp("two-path"));
    let a = repo("two-a");
    let b = repo("two-b");
    pin(&b, "7.7.7");

    let ra = run(&a, &path, &home, &[], &["lint"]);
    assert_eq!(ra.code, 0, "{}{}", ra.out, ra.err);
    assert!(ra.out.contains("-- 0 error(s)"), "{}", ra.out);
    let rb = run(&b, &path, &home, &[], &["lint"]);
    assert_eq!(rb.code, 0, "{}{}", rb.out, rb.err);
    assert_eq!(rb.out, "docsys 7.7.7 ran: lint\nguard=7.7.7\n");
    // from a subdirectory too: the tree is found first, then its pin
    fs::create_dir_all(b.join("src")).unwrap();
    let rs = run(&b.join("src"), &path, &home, &[], &["status"]);
    assert_eq!(rs.out, "docsys 7.7.7 ran: status\nguard=7.7.7\n");

    // the relays and the gate of tree B run 7.7.7; tree A's run this build
    for r in [&a, &b] {
        let out = Command::new(bin())
            .args(["adopt"])
            .current_dir(r)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    }
    let relay = |r: &Path| -> String {
        let mut child = Command::new("bash")
            .arg(r.join(".claude/hooks/session-intent.sh"))
            .current_dir(r)
            .env("PATH", &path)
            .env("DOCSYS_HOME", &home)
            .env("CLAUDE_PROJECT_DIR", r)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write as _;
        child.stdin.take().unwrap().write_all(b"{}").unwrap();
        let o = child.wait_with_output().unwrap();
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    assert!(
        relay(&b).starts_with("docsys 7.7.7 ran: hook user-prompt-submit"),
        "{}",
        relay(&b)
    );
    assert!(!relay(&a).contains("ran:"), "{}", relay(&a));
    fs::write(b.join("notes.txt"), "a change\n").unwrap();
    git(&b, &["add", "-A"]);
    let commit = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "through the gate",
        ])
        .current_dir(&b)
        .env("PATH", &path)
        .env("DOCSYS_HOME", &home)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&commit.stdout).into_owned()
        + &String::from_utf8_lossy(&commit.stderr);
    assert!(commit.status.success(), "{said}");
    assert!(said.contains("docsys 7.7.7 ran: gate"), "{said}");
    for d in [a, b, home] {
        let _ = fs::remove_dir_all(d);
    }
}

/// A teammate pulls a pin change: the next call installs the new version
/// once, through cargo, into the cache, and runs it.
#[test]
fn a_pulled_pin_change_installs_the_new_version_once() {
    let home = tmp("pull-home");
    let stubs = tmp("pull-stubs");
    let log = stubs.join("cargo.log");
    stub_cargo(&stubs, &log);
    let path = path_with(&stubs);
    let r = repo("pull");
    pin(&r, "0.17.3");

    let first = run(&r, &path, &home, &[], &["lint"]);
    assert_eq!(first.code, 0, "{}{}", first.out, first.err);
    assert_eq!(first.out, "docsys 0.17.3 ran: lint\n");
    assert!(
        first
            .err
            .contains("docsys: this tree pins docsys 0.17.3; installing it once…"),
        "{}",
        first.err
    );
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        format!(
            "install docsys --version 0.17.3 --locked --root {}\n",
            home.join("versions/0.17.3").display()
        )
    );
    let again = run(&r, &path, &home, &[], &["lint"]);
    assert_eq!(again.out, "docsys 0.17.3 ran: lint\n");
    assert!(again.err.is_empty(), "{}", again.err);
    assert_eq!(fs::read_to_string(&log).unwrap().lines().count(), 1);
    for d in [r, home, stubs] {
        let _ = fs::remove_dir_all(d);
    }
}

/// No cargo, or a cargo that cannot reach the registry: the exact command,
/// exit 1, nothing half-installed — and an unpinned tree does not notice.
#[test]
fn without_cargo_or_offline_the_command_is_named_and_nothing_runs() {
    let home = tmp("off-home");
    let r = repo("off");
    pin(&r, "0.17.3");
    let command = format!(
        "cargo install docsys --version 0.17.3 --locked --root {}",
        home.join("versions/0.17.3").display()
    );

    let none = path_with(&tmp("off-none"));
    let x = run(&r, &none, &home, &[], &["lint"]);
    assert_eq!(x.code, 1, "{}{}", x.out, x.err);
    assert_eq!(
        x.err,
        format!("docsys: this tree pins docsys 0.17.3; install it: {command}\n")
    );
    assert!(x.out.is_empty(), "{}", x.out);

    let stubs = tmp("off-stubs");
    offline_cargo(&stubs);
    let x = run(&r, &path_with(&stubs), &home, &[], &["lint"]);
    assert_eq!(x.code, 1, "{}{}", x.out, x.err);
    assert_eq!(
        x.err.lines().last(),
        Some(
            format!("docsys: version 0.17.3 could not be installed; install it: {command}")
                .as_str()
        ),
        "{}",
        x.err
    );
    assert!(!home.join("versions/0.17.3/bin/docsys").exists());

    // a tree 0.15 wrote has no pin: this build serves it, cargo or not
    let old = repo("off-old");
    fs::remove_file(old.join("docs/.docsys-version")).unwrap();
    let x = run(&old, &none, &home, &[], &["lint"]);
    assert_eq!(x.code, 0, "{}{}", x.out, x.err);
    for d in [r, old, home, stubs] {
        let _ = fs::remove_dir_all(d);
    }
}

/// The opt-out, and an agent hook: the command is named, cargo is never
/// asked, and the hook exits 1 — the harness's non-blocking failure.
#[test]
fn the_opt_out_and_a_hook_name_the_command_and_install_nothing() {
    let home = tmp("opt-home");
    let stubs = tmp("opt-stubs");
    let log = stubs.join("cargo.log");
    stub_cargo(&stubs, &log);
    let path = path_with(&stubs);
    let r = repo("opt");
    pin(&r, "0.17.3");
    let line = format!(
        "docsys: this tree pins docsys 0.17.3; install it: cargo install docsys --version 0.17.3 --locked --root {}\n",
        home.join("versions/0.17.3").display()
    );

    let x = run(
        &r,
        &path,
        &home,
        &[("DOCSYS_NO_AUTO_INSTALL", "1")],
        &["lint"],
    );
    assert_eq!((x.code, x.err.as_str()), (1, line.as_str()), "{}", x.out);
    let x = run(
        &r,
        &path,
        &home,
        &[],
        &["hook", "stop", "--root", "docs", "--stdin"],
    );
    assert_eq!((x.code, x.err.as_str()), (1, line.as_str()), "{}", x.out);
    assert!(!log.exists(), "cargo was asked");
    for d in [r, home, stubs] {
        let _ = fs::remove_dir_all(d);
    }
}

/// The cache's binary for the pin is this dispatcher itself: it runs once,
/// as itself, and returns.
#[test]
fn a_pinned_binary_that_dispatches_too_does_not_loop() {
    let home = tmp("loop-home");
    let cached = home.join("versions/9.9.9/bin/docsys");
    fs::create_dir_all(cached.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(bin(), &cached).unwrap();
    let r = repo("loop");
    pin(&r, "9.9.9");
    let x = run(&r, &path_with(&tmp("loop-path")), &home, &[], &["lint"]);
    assert_eq!(x.code, 0, "{}{}", x.out, x.err);
    assert_eq!(x.out.matches("-- 0 error(s)").count(), 1, "{}", x.out);
    for d in [r, home] {
        let _ = fs::remove_dir_all(d);
    }
}

/// `--version` names the pin inside a pinned tree.
#[test]
fn version_names_the_running_docsys_and_the_pin() {
    let home = tmp("ver-home");
    let r = repo("ver");
    pin(&r, "0.17.3");
    let x = run(&r, &path_with(&tmp("ver-path")), &home, &[], &["--version"]);
    assert_eq!(
        x.out,
        format!(
            "docsys {} (spec docsys/{}); this tree pins docsys 0.17.3\n",
            env!("CARGO_PKG_VERSION"),
            docsys::rules::spec_version()
        )
    );
    for d in [r, home] {
        let _ = fs::remove_dir_all(d);
    }
}

/// The guard stops a dispatched binary from dispatching again, and nothing
/// more: the git gate a dispatched `verify --commit` starts resolves the pin
/// itself, as a gate under a plain `git commit` does. `verify` is a
/// docsys/0.4 tree's command (D-130), so the tree declares 0.4.
#[test]
fn the_children_of_a_dispatched_command_resolve_the_pin_themselves() {
    let home = tmp("child-home");
    let log = home.join("ran.log");
    executable(
        &home.join("versions/9.9.9/bin/docsys"),
        &format!(
            "#!/bin/sh\necho \"$1\" >> '{}'\nexec '{}' \"$@\"\n",
            log.display(),
            bin().display()
        ),
    );
    let path = path_with(&tmp("child-path"));
    let r = repo("child");
    let adopt = Command::new(bin())
        .arg("adopt")
        .current_dir(&r)
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(adopt.status.success(), "{adopt:?}");
    let meta = r.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&meta).unwrap();
    fs::write(&meta, text.replace("spec: docsys/0.5", "spec: docsys/0.4")).unwrap();
    fs::create_dir_all(r.join("docs/reference")).unwrap();
    fs::create_dir_all(r.join("src")).unwrap();
    fs::write(r.join("src/p.rs"), "pub fn p() {}\n").unwrap();
    fs::write(
        r.join("docs/reference/p.md"),
        "---\nid: p\ntype: reference\nverification: unverified\nsources: [src/p.rs]\n---\n# P\n\nThis page states one fact; read it first.\n",
    )
    .unwrap();
    fs::write(r.join("docs/.docsys-version"), "9.9.9\n").unwrap();
    // setup only: these commits are not what the test watches
    git(&r, &["add", "-A"]);
    git(
        &r,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-qm",
            "a page, pinned",
        ],
    );
    let _ = fs::remove_file(&log);
    let x = run(&r, &path, &home, &[], &["verify", "p", "--commit"]);
    assert_eq!(x.code, 0, "{}{}", x.out, x.err);
    let ran = fs::read_to_string(&log).unwrap_or_default();
    // verify itself, then the gate's own calls inside its commit
    assert!(ran.starts_with("verify\n"), "{ran}");
    assert!(
        ran.lines().any(|l| l == "gate"),
        "the gate ran unpinned:\n{ran}"
    );
    for d in [r, home] {
        let _ = fs::remove_dir_all(d);
    }
}

/// "Which pages describe this file": `backlinks` with a code path lists the
/// pages that pin it, symbol by symbol, and the pages whose sources name it —
/// the binding's other direction, with nothing written into the code (N0).
#[test]
fn backlinks_of_a_code_file_names_the_pages_that_describe_it() {
    let home = tmp("describe-home");
    let path = path_with(&tmp("describe-path"));
    let r = repo("describe");
    fs::create_dir_all(r.join("src")).unwrap();
    fs::write(
        r.join("src/retry.rs"),
        "pub fn delay_for(a: u32) -> u32 {\n    a * 2\n}\n\npub fn should_retry(a: u32) -> bool {\n    a < 6\n}\n",
    )
    .unwrap();
    fs::create_dir_all(r.join("docs/reference")).unwrap();
    fs::write(
        r.join("docs/reference/retry.md"),
        "---\nid: retry\ntype: reference\nverification: unverified\nsources: [src/retry.rs]\n---\n# Retry\n\nThis page states the retry policy; read it first.\n",
    )
    .unwrap();
    git(&r, &["add", "-A"]);
    git(
        &r,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-qm",
            "code and a page",
        ],
    );
    let x = run(
        &r,
        &path,
        &home,
        &[],
        &[
            "pin",
            "reference/retry",
            "src/retry.rs",
            "--symbol",
            "should_retry",
        ],
    );
    assert_eq!(x.code, 0, "{}{}", x.out, x.err);
    // from a subdirectory, the path as the shell completes it there
    let x = run(
        &r.join("src"),
        &path,
        &home,
        &[],
        &["backlinks", "retry.rs"],
    );
    assert_eq!(x.code, 0, "{}{}", x.out, x.err);
    assert!(
        x.out.contains("# pages that describe src/retry.rs"),
        "{}",
        x.out
    );
    assert!(
        x.out
            .contains("reference/retry.md pins src/retry.rs#should_retry"),
        "{}",
        x.out
    );
    assert!(
        x.out.contains("reference/retry.md rests on it (sources:)"),
        "{}",
        x.out
    );
    for d in [r, home] {
        let _ = fs::remove_dir_all(d);
    }
}
