use docsys::{migrate, to_json, Outcome};
use std::path::PathBuf;
use std::process::ExitCode;

/// Standard output, written as `print!` writes it — but a reader that closed
/// its end (`| head`) ends the command, as a closed pipe ends any program,
/// except inside a git hook, where only the findings decide (`closed_reader`).
fn write_stdout(args: std::fmt::Arguments) {
    use std::io::Write;
    match std::io::stdout().write_fmt(args) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => docsys::closed_reader(),
        Err(e) => {
            eprintln!("docsys: standard output: {e}");
            std::process::exit(2)
        }
    }
}

// every `eprint!` and `eprintln!` below goes through the library's writer
macro_rules! eprint {
    ($($arg:tt)*) => { docsys::write_stderr(format_args!($($arg)*)) };
}

macro_rules! eprintln {
    () => { docsys::write_stderr(format_args!("\n")) };
    ($($arg:tt)*) => { docsys::write_stderr(format_args!("{}\n", format_args!($($arg)*))) };
}

// every `print!` and `println!` below goes through `write_stdout`
macro_rules! print {
    ($($arg:tt)*) => { write_stdout(format_args!($($arg)*)) };
}

macro_rules! println {
    () => { write_stdout(format_args!("\n")) };
    ($($arg:tt)*) => { write_stdout(format_args!("{}\n", format_args!($($arg)*))) };
}

struct Opts {
    root: PathBuf,
    json: bool,
    lang: String,
    lang_explicit: bool,
    title: Option<String>,
    follow: bool,
    audience: Option<String>,
    profile: Option<String>,
    kb: bool,
    plan: Option<PathBuf>,
    /// `rules --write <file>`
    write: Option<PathBuf>,
    out: Option<PathBuf>,
    repo: Option<PathBuf>,
    dir: PathBuf,
    force: bool,
    unverified: bool,
    stdin: bool,
    skipped: bool,
    by: Option<String>,
    confirmed: Option<String>,
    commit: bool,
    revoke: bool,
    show: bool,
    from_trailers: bool,
    target: Option<String>,
    since: Option<String>,
    memory: Option<PathBuf>,
    note: Option<String>,
    message: Option<PathBuf>,
    deferred: Option<String>,
    repay_when: Option<String>,
    answer: Option<String>,
    approval: Option<String>,
    topic: Option<String>,
    context: Option<String>,
    date: Option<String>,
    link: Option<String>,
    format: Option<String>,
    obsidian: bool,
    agents_md: bool,
    procedures: bool,
    report: bool,
    max_lines: usize,
    range: Option<String>,
    refresh: bool,
    gc: bool,
    symbol: Option<String>,
    block: Option<usize>,
    as_ns: Option<String>,
    source: Option<String>,
    source_id: Option<String>,
    url: Option<String>,
    limit: Option<usize>,
    all: bool,
    projects: Vec<PathBuf>,
    domains: Vec<String>,
    rules_file: Option<PathBuf>,
    report_dir: Option<PathBuf>,
    no_report: bool,
    draft: bool,
    kind: Option<String>,
    rule: Option<String>,
    command: Option<String>,
    apply: bool,
    ci_runner: Option<String>,
    ci_install: Option<String>,
    ci_sha256: Option<String>,
    verify_on_approval: Option<String>,
    positional: Vec<String>,
}

/// A flag's value: the next argument, never another flag.
fn value<'a>(it: &mut std::slice::Iter<'a, String>, need: &str) -> Result<&'a String, String> {
    match it.next() {
        Some(v) if v.starts_with("--") => Err(format!("{need}, not the flag `{v}`")),
        Some(v) => Ok(v),
        None => Err(need.to_string()),
    }
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        root: PathBuf::from("docs"),
        json: false,
        lang: "en".to_string(),
        lang_explicit: false,
        title: None,
        follow: false,
        audience: None,
        profile: None,
        kb: false,
        plan: None,
        write: None,
        out: None,
        repo: None,
        dir: PathBuf::from(".claude"),
        force: false,
        unverified: false,
        stdin: false,
        skipped: false,
        by: None,
        confirmed: None,
        commit: false,
        revoke: false,
        show: false,
        from_trailers: false,
        target: None,
        since: None,
        memory: None,
        note: None,
        message: None,
        deferred: None,
        repay_when: None,
        answer: None,
        approval: None,
        topic: None,
        context: None,
        date: None,
        link: None,
        format: None,
        obsidian: false,
        agents_md: false,
        procedures: false,
        report: false,
        max_lines: 200,
        range: None,
        refresh: false,
        gc: false,
        symbol: None,
        block: None,
        as_ns: None,
        source: None,
        source_id: None,
        url: None,
        limit: None,
        all: false,
        projects: Vec::new(),
        domains: Vec::new(),
        rules_file: None,
        report_dir: None,
        no_report: false,
        draft: false,
        kind: None,
        rule: None,
        command: None,
        apply: false,
        ci_runner: None,
        ci_install: None,
        ci_sha256: None,
        verify_on_approval: None,
        positional: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        // a value flag takes the next argument, never another flag; the table
        // says which flags take one (D-129)
        let v = if docsys::help::takes_value(a) {
            let need = match a.as_str() {
                "--message" => "--message needs a file".to_string(),
                "--approval" => "--approval needs a @login".to_string(),
                "--block" => "--block needs a block number".to_string(),
                other => format!("{other} needs a value"),
            };
            value(&mut it, &need)?.clone()
        } else {
            String::new()
        };
        let val = || v.clone();
        match a.as_str() {
            "--root" => o.root = PathBuf::from(val()),
            "--lang" => {
                o.lang = val();
                o.lang_explicit = true;
            }
            "--plan" => o.plan = Some(PathBuf::from(val())),
            "--out" => o.out = Some(PathBuf::from(val())),
            "--title" => o.title = Some(val()),
            "--follow" => o.follow = true,
            "--audience" => o.audience = Some(val()),
            "--profile" => o.profile = Some(val()),
            "--kb" => o.kb = true,
            "--repo" => o.repo = Some(PathBuf::from(val())),
            "--dir" => o.dir = PathBuf::from(val()),
            "--force" => o.force = true,
            "--unverified" => o.unverified = true,
            "--stdin" => o.stdin = true,
            "--by" => o.by = Some(val()),
            "--confirmed" => {
                o.confirmed = Some(val());
            }
            "--commit" => o.commit = true,
            "--revoke" => o.revoke = true,
            "--show" => o.show = true,
            "--from-trailers" => o.from_trailers = true,
            "--skipped" => o.skipped = true,
            "--target" => o.target = Some(val()),
            "--since" => o.since = Some(val()),
            "--memory" => o.memory = Some(PathBuf::from(val())),
            "--note" => o.note = Some(val()),
            "--message" => o.message = Some(PathBuf::from(val())),
            "--deferred" => o.deferred = Some(val()),
            "--repay-when" => o.repay_when = Some(val()),
            "--answer" => o.answer = Some(val()),
            "--approval" => o.approval = Some(val()),
            "--topic" => o.topic = Some(val()),
            "--context" => o.context = Some(val()),
            "--reason" => o.note = Some(val()),
            "--date" => o.date = Some(val()),
            "--link" => o.link = Some(val()),
            "--format" => o.format = Some(val()),
            "--obsidian" => o.obsidian = true,
            "--draft" => o.draft = true,
            "--apply" => o.apply = true,
            "--type" => o.kind = Some(val()),
            "--rule" => o.rule = Some(val()),
            "--command" => o.command = Some(val()),
            "--agents-md" => o.agents_md = true,
            "--write" => o.write = Some(PathBuf::from(val())),
            "--report" => o.report = true,
            "--procedures" => o.procedures = true,
            "--max-lines" => {
                o.max_lines = val()
                    .parse()
                    .map_err(|_| "--max-lines needs a number".to_string())?;
            }
            "--json" => o.json = true,
            "--range" => o.range = Some(val()),
            "--refresh" => o.refresh = true,
            "--gc" => o.gc = true,
            "--symbol" => o.symbol = Some(val()),
            "--block" => {
                o.block = Some(val().parse().map_err(|_| {
                    "--block needs a block number, as `docsys verify --show <page>` numbers them"
                        .to_string()
                })?);
            }
            "--as" => o.as_ns = Some(val()),
            "--source" => o.source = Some(val()),
            "--id" => o.source_id = Some(val()),
            "--url" => o.url = Some(val()),
            "--all" => o.all = true,
            "--rules-file" => o.rules_file = Some(PathBuf::from(val())),
            "--report-dir" => o.report_dir = Some(PathBuf::from(val())),
            "--no-report" => o.no_report = true,
            "--ci-runner" => o.ci_runner = Some(val()),
            "--ci-install" => o.ci_install = Some(val()),
            "--ci-sha256" => o.ci_sha256 = Some(val()),
            "--verify-on-approval" => o.verify_on_approval = Some(val()),
            "--projects" => o.projects.push(PathBuf::from(val())),
            "--domains" => {
                o.domains = val()
                    .split(',')
                    .map(|d| d.trim().to_string())
                    .filter(|d| !d.is_empty())
                    .collect()
            }
            "--limit" => {
                o.limit = Some(
                    val()
                        .parse()
                        .map_err(|_| "--limit needs a number".to_string())?,
                );
            }
            other if !other.starts_with("--") => o.positional.push(other.to_string()),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(o)
}

fn run_lint(o: &Opts) -> ExitCode {
    // The repository is where pins resolve and history lives: the top level of
    // the one given, or of the one the root sits in (D-098). Outside any
    // repository the tree is linted alone.
    let repo = match &o.repo {
        Some(r) => Some(docsys::repo_of(r).unwrap_or_else(|| r.clone())),
        None => docsys::repo_of(&o.root),
    };
    let (report, outcome) = docsys::lint_in(&o.root, repo.as_deref());
    if o.json {
        print!("{}", to_json(&report));
    } else {
        for f in &report.findings {
            println!(
                "{} {} {} [{}] {}",
                f.severity.tag(),
                f.rule,
                f.file,
                f.subject,
                f.message
            );
        }
        let errors = report
            .findings
            .iter()
            .filter(|f| f.severity == docsys::model::Severity::Error)
            .count();
        let warns = report.findings.len() - errors;
        let units: usize = report.inspected.values().sum();
        println!("-- {errors} error(s), {warns} warning(s); {units} unit(s) inspected");
        if docsys::era::Era::at(&o.root).finding_pointers() {
            for p in
                docsys::feedback::pointers_once(&o.root, report.findings.iter().map(|f| f.rule.0))
            {
                println!("{p}");
            }
        }
    }
    match outcome {
        Outcome::Clean => ExitCode::SUCCESS,
        Outcome::Errors => ExitCode::from(1),
        Outcome::Config => ExitCode::from(2),
    }
}

/// Print/write a composed export. An unchanged `--out` file is left
/// untouched so nothing downstream re-triggers on a no-op.
fn emit_export(done: &docsys::export::ProductOutcome, out: Option<&std::path::Path>) -> ExitCode {
    for w in &done.warnings {
        eprintln!("WARN {w}");
    }
    match out {
        Some(path) => match docsys::export::write_if_changed(path, &done.output) {
            Ok(true) => {
                eprintln!("composed {} page(s) -> {}", done.pages, path.display());
                ExitCode::SUCCESS
            }
            Ok(false) => {
                eprintln!("unchanged — {} left untouched", path.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("export: {e}");
                ExitCode::from(2)
            }
        },
        None => {
            print!("{}", done.output);
            ExitCode::SUCCESS
        }
    }
}

/// `arg` as a file of the repository, relative to its top: the path as the
/// shell completes it where the command runs, or relative to the top.
fn code_file(arg: &str, repo: &std::path::Path) -> Option<String> {
    let top = repo.canonicalize().ok()?;
    let here = std::env::current_dir().ok()?.join(arg);
    let path = here
        .canonicalize()
        .ok()
        .filter(|p| p.is_file())
        .or_else(|| top.join(arg).canonicalize().ok().filter(|p| p.is_file()))?;
    path.strip_prefix(&top)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

/// `migrate`'s root and repository (D-098): a given `--repo` is its top level;
/// a given `--root` names a directory from the repository's top, and without
/// one the tree found from where the command stands — the same from anywhere
/// in the repository.
fn migrate_paths(opts: &Opts, root_given: bool) -> (PathBuf, Option<PathBuf>) {
    let cwd = std::env::current_dir().ok();
    let given = opts.repo.as_deref();
    let top = given
        .map(docsys::place::repo_top)
        .or_else(|| cwd.as_deref().and_then(docsys::git::toplevel));
    let canon = |p: &std::path::Path| p.canonicalize().ok();
    let at_top = match (
        cwd.as_deref().and_then(canon),
        top.as_deref().and_then(canon),
    ) {
        (Some(c), Some(t)) => c == t,
        _ => false,
    };
    let tree = (!root_given)
        .then(|| docsys::place::locate(&docsys::place::cwd_anchor(), &opts.root, None).root)
        .filter(|t| t.join(".docmeta.yml").is_file());
    let root = match (tree, &top) {
        (Some(tree), _) => tree,
        (None, Some(top)) if !at_top && opts.root.is_relative() => top.join(&opts.root),
        _ => opts.root.clone(),
    };
    (root, given.and(top))
}

/// The tree `agents` writes its relays for, relative to the repository: the
/// root given when it is a tree, else the repository's one tree (D-098, D-099).
fn agents_root(opts: &Opts) -> PathBuf {
    let repo = opts
        .dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    if opts.root.is_absolute() || repo.join(&opts.root).join(".docmeta.yml").is_file() {
        return opts.root.clone();
    }
    docsys::git::toplevel(repo)
        .and_then(|top| {
            let tree = docsys::place::only_tree(&top)?;
            let rel = tree.strip_prefix(&top).ok()?.to_path_buf();
            Some(if rel.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                rel
            })
        })
        .unwrap_or_else(|| opts.root.clone())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // the command table reads the line: what it names, and what it refuses
    // (D-129)
    let (entry, words, from) = match docsys::help::read(&args) {
        Ok(docsys::help::Read::Run { entry, words, from }) => (entry, words, from),
        Ok(docsys::help::Read::Help(text)) => {
            print!("{text}");
            return ExitCode::SUCCESS;
        }
        Err(r) => {
            if !r.line.is_empty() {
                eprintln!("{}", r.line);
            }
            eprint!("{}", r.entries);
            return ExitCode::from(2);
        }
    };
    let rest: &[String] = args.get(from..).unwrap_or_default();
    let (cmd, sub): (&str, Option<&str>) = match entry.name.split_once(' ') {
        Some((c, s)) => (c, Some(s)),
        // a hook's event is its sub-command here
        None if entry.name == "hook" => (entry.name, words.first().map(String::as_str)),
        None => (entry.name, None),
    };
    let root_given = rest.iter().any(|a| a == "--root");
    let mut opts = match parse_opts(rest) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprint!("{}", docsys::help::of(&[entry.name]).unwrap_or_default());
            return ExitCode::from(2);
        }
    };
    opts.positional.clone_from(&words);
    // `agents` installs into the agent layer at the repository's top, wherever it
    // runs: a relative --dir, given or not, names it there (D-098)
    if cmd == "agents" && opts.dir.is_relative() {
        let cwd = std::env::current_dir().and_then(|d| d.canonicalize()).ok();
        let top = cwd
            .as_deref()
            .and_then(docsys::git::toplevel)
            .and_then(|t| t.canonicalize().ok());
        if let (Some(cwd), Some(top)) = (cwd, top) {
            if cwd != top {
                opts.dir = top.join(&opts.dir);
            }
        }
    }
    // A command that works on an existing tree finds it from where it stands
    // and takes the repository from the tree (D-098); one that creates a tree,
    // or installs into one, takes its paths as given. A hook finds its own.
    let creates = matches!(
        cmd,
        "init"
            | "adopt"
            | "assistant"
            | "migrate"
            | "agents"
            | "rules"
            | "hook"
            | "help"
            | "--help"
            | "-h"
            | "--version"
            | "-V"
            | "version"
            | ""
    );
    let here = (!creates).then(|| {
        docsys::place::locate(
            &docsys::place::cwd_anchor(),
            &opts.root,
            opts.repo.as_deref(),
        )
    });
    if let Some(p) = &here {
        opts.root = p.root.clone();
        // every command reads the repository from here: a given --repo is its
        // top level, and a relative --dir the agent layer there (D-098)
        if let Some(top) = &p.repo {
            if opts.repo.is_some() {
                opts.repo = Some(top.clone());
            }
            if opts.dir.is_relative() && top.as_path() != std::path::Path::new(".") {
                opts.dir = top.join(&opts.dir);
            }
        }
    }
    // A pinned tree runs its own docsys (D-120). `upgrade` is how a newer one
    // moves the pin; a hook dispatches too, but never waits for an install.
    // Inside a git hook a gate chains several docsys calls — this version's
    // block and the one 0.15 wrote alike — and only `gate` says what concerns
    // the commit as a whole: the version notice and a pin that cannot run.
    // The commit-msg half of a docsys/0.5 gate follows the pre-commit half,
    // which has said it already (D-125) — but for a merge, which runs the
    // commit-msg half alone.
    let merging = opts
        .message
        .as_deref()
        .and_then(std::path::Path::file_name)
        .is_some_and(|n| n == "MERGE_MSG");
    let quiet = std::env::var_os("GIT_EXEC_PATH").is_some()
        && std::env::var_os("GIT_INDEX_FILE").is_some()
        && (cmd != "gate" || (opts.message.is_some() && !merging));
    let pinned_root = match cmd {
        "upgrade" => None,
        "hook" => Some(
            docsys::place::locate(
                &docsys::place::cwd_anchor(),
                &opts.root,
                opts.repo.as_deref(),
            )
            .root,
        ),
        _ => here.as_ref().map(|p| p.root.clone()),
    };
    if let Some(root) = pinned_root {
        if let Err(code) = docsys::dispatch::run_pinned(&root, cmd != "hook", quiet) {
            return code;
        }
    }
    // R-171: a version difference in one line, naming what resolves it; once
    // per commit inside a git hook (above). A tree that has not moved is
    // served by its own rules (D-118).
    if let Some(p) = here
        .as_ref()
        .filter(|p| p.root.join(".docmeta.yml").is_file())
    {
        let tree = docsys::era::Era::at(&p.root).0;
        let ours = docsys::upgrade::implemented();
        if quiet {
        } else if cmd != "upgrade" && tree < ours {
            eprintln!("docsys: this tree declares docsys/0.{tree} and is served by its rules; `docsys upgrade` — or `/docsys-upgrade` with an agent — moves it to docsys/0.{ours} when the repository is ready");
        } else if tree > ours {
            eprintln!("docsys: this tree declares docsys/0.{tree}; this docsys implements docsys/0.{ours} — install a newer docsys");
        }
    }
    // a command that writes into a tree refuses where there is none: it never
    // makes a second, half tree (R-160, D-098)
    if matches!(
        (cmd, sub),
        ("debt" | "question", Some("add" | "close"))
            | ("page", Some("new"))
            | ("journal", Some("add"))
    ) && !opts.root.join(".docmeta.yml").is_file()
    {
        eprintln!(
            "ERROR R-160 - [root] `{}` is no documentation tree (no .docmeta.yml) — `docsys adopt` or `docsys init` makes one, `--root` names another",
            opts.root.display()
        );
        return ExitCode::from(2);
    }
    let repo_or_cwd = here
        .as_ref()
        .and_then(|p| p.repo.clone())
        .or_else(|| opts.repo.clone())
        .unwrap_or_else(|| PathBuf::from("."));
    match (cmd, sub) {
        ("help", None) => {
            print!("{}", docsys::help::overview());
            ExitCode::SUCCESS
        }
        ("version", None) => {
            // the relays and the git gate ask it; inside a pinned tree it names
            // the pin too (D-120)
            let root = docsys::place::locate(&docsys::place::cwd_anchor(), &opts.root, None).root;
            let pin = docsys::dispatch::read(&root)
                .ok()
                .flatten()
                .map(|v| format!("; this tree pins docsys {v}"))
                .unwrap_or_default();
            println!(
                "docsys {} (spec docsys/{}){pin}",
                env!("CARGO_PKG_VERSION"),
                docsys::rules::spec_version()
            );
            ExitCode::SUCCESS
        }
        ("lint", None) => run_lint(&opts),
        ("upgrade", None) => {
            let repo = repo_or_cwd.clone();
            if docsys::repo_of(&repo).is_none() {
                eprintln!("upgrade: a tree moves inside a git repository — its move is one commit (R-177)");
                return ExitCode::from(2);
            }
            let dir = if opts.dir.is_relative() && repo.as_path() != std::path::Path::new(".") {
                repo.join(&opts.dir)
            } else {
                opts.dir.clone()
            };
            // a move written and not yet committed is recorded beside the
            // index — its message and its files — so nothing between the two
            // loses it, and a re-run commits it (R-177)
            let git_path = |name: &str| {
                docsys::git::cmd(&repo)
                    .args(["rev-parse", "--git-path", name])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
                    .map(|p| if p.is_relative() { repo.join(p) } else { p })
                    .unwrap_or_else(|| repo.join(".git").join(name))
            };
            let message_path = git_path("docsys-upgrade-message");
            let files_path = git_path("docsys-upgrade-files");
            // the commit the record was written on: a record from an earlier
            // HEAD was committed by hand, or abandoned, and is no move now
            let head_path = git_path("docsys-upgrade-head");
            let head = || {
                docsys::git::cmd(&repo)
                    .args(["rev-parse", "-q", "--verify", "HEAD"])
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_default()
            };
            let current = std::fs::read_to_string(&head_path).is_ok_and(|h| h.trim() == head());
            if !current {
                let _ = std::fs::remove_file(&message_path);
                let _ = std::fs::remove_file(&files_path);
                let _ = std::fs::remove_file(&head_path);
            }
            let changed: Option<Vec<String>> = docsys::git::cmd(&repo)
                .args(["status", "--porcelain", "--untracked-files=no"])
                .output()
                .ok()
                .map(|o| {
                    docsys::hook::porcelain_paths(
                        &String::from_utf8_lossy(&o.stdout)
                            .lines()
                            .map(str::to_string)
                            .collect::<Vec<_>>(),
                    )
                });
            let dirty = changed.as_ref().is_none_or(|c| !c.is_empty());
            let recorded: Vec<String> = std::fs::read_to_string(&files_path)
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect();
            // a move written and waiting holds only its own files; anything
            // else changed is the person's, never the move's commit
            // a recorded directory (`.verifies/`) holds the files under it
            let outside: Vec<String> = changed
                .unwrap_or_default()
                .into_iter()
                .filter(|f| {
                    !recorded
                        .iter()
                        .any(|r| f == r || f.starts_with(&format!("{}/", r.trim_end_matches('/'))))
                })
                .collect();
            // a recorded move stays the move's, `--force` or not: what lies
            // outside it decides only the refusal, and the commit takes the
            // move's paths alone
            let pending = dirty && message_path.is_file() && files_path.is_file();
            if !dirty {
                let _ = std::fs::remove_file(&message_path);
                let _ = std::fs::remove_file(&files_path);
                let _ = std::fs::remove_file(&head_path);
            }
            if opts.apply && !opts.force && dirty && !(pending && outside.is_empty()) {
                let named = if recorded.is_empty() || outside.is_empty() {
                    "uncommitted changes".to_string()
                } else {
                    format!("changes outside the move: {}", outside.join(", "))
                };
                eprintln!("upgrade: the working tree has {named} — commit or stash them first, so the upgrade is one commit of its own (R-097, R-177); --force overrides");
                return ExitCode::from(2);
            }
            // one move per round, each its own commit (R-177); with --commit the
            // rounds run until the tree is where this docsys is
            let mut pending = pending;
            loop {
                let u = match docsys::upgrade::run(&repo, &opts.root, &dir, opts.apply) {
                    Ok(u) => u,
                    Err(e) => {
                        eprintln!("upgrade: {e}; nothing is committed — `git status` lists what the move wrote");
                        return ExitCode::from(2);
                    }
                };
                // the move and its commit come before any word of output, so
                // a reader that leaves early cannot stand between them
                let mut committed = None;
                if opts.apply {
                    let mut files: Vec<String> = if pending {
                        std::fs::read_to_string(&files_path)
                            .unwrap_or_default()
                            .lines()
                            .map(str::to_string)
                            .collect()
                    } else {
                        Vec::new()
                    };
                    for f in &u.written {
                        if !files.contains(f) {
                            files.push(f.clone());
                        }
                    }
                    let message = match std::fs::read_to_string(&message_path) {
                        Ok(m) if pending => m,
                        _ => docsys::upgrade::message(&u),
                    };
                    if !files.is_empty() {
                        let recorded = std::fs::write(&message_path, &message)
                            .and_then(|()| std::fs::write(&files_path, files.join("\n") + "\n"))
                            .and_then(|()| std::fs::write(&head_path, format!("{}\n", head())));
                        if let Err(e) = recorded {
                            eprintln!("upgrade: {}: {e}", message_path.display());
                            return ExitCode::from(1);
                        }
                    }
                    if opts.commit {
                        if let Err(e) = docsys::upgrade::commit_files(&repo, &files, &message) {
                            eprintln!("upgrade: {e}");
                            eprintln!(
                                "the move is written and waits for its commit: once git takes it, `docsys upgrade --apply --commit` commits it, or stage what it wrote and `git commit -F {}`",
                                docsys::place::shown(&message_path).display()
                            );
                            return ExitCode::from(1);
                        }
                        let _ = std::fs::remove_file(&message_path);
                        let _ = std::fs::remove_file(&files_path);
                        let _ = std::fs::remove_file(&head_path);
                        pending = false;
                        if !files.is_empty() {
                            committed = Some(message.lines().next().unwrap_or("").to_string());
                        }
                    }
                }
                // --json: the plan as data, one object per move, nothing else
                if opts.json {
                    println!("{}", u.to_json());
                } else {
                    let head = if u.from < u.to {
                        format!("docsys/0.{} → docsys/0.{}", u.from, u.to)
                    } else {
                        format!("docsys/0.{}", u.to)
                    };
                    println!(
                        "docsys upgrade: {head} — {}",
                        if opts.apply {
                            "applied"
                        } else {
                            "the plan; `docsys upgrade --apply` writes it"
                        }
                    );
                    for (release, text) in &u.notes {
                        println!("\nUpgrading to docsys {release}:\n{text}\n");
                    }
                    for i in &u.items {
                        let command = i
                            .command
                            .as_deref()
                            .map(|c| format!(": `{c}`"))
                            .unwrap_or_default();
                        println!(
                            "{:<7} {:<18} {}  {}{command}",
                            i.strategy, i.step, i.file, i.what
                        );
                    }
                    let count = |s: &str| u.items.iter().filter(|i| i.strategy == s).count();
                    if u.items.is_empty() {
                        println!("-- nothing to do");
                    } else {
                        println!(
                            "-- {} automatic, {} for a person, {} for information",
                            count("auto"),
                            count("manual"),
                            count("info")
                        );
                    }
                    // the findings the move adds and takes away: a forecast,
                    // the plan's alone
                    if !opts.apply {
                        for p in &u.preview {
                            println!("{p}");
                        }
                    }
                    for (file, diff) in &u.diffs {
                        println!("\n# {file}\n{diff}");
                    }
                    if let Some(subject) = &committed {
                        println!("committed: {subject}");
                    }
                }
                if opts.apply && opts.commit {
                    if !u.last {
                        continue;
                    }
                } else if opts.apply && message_path.is_file() && !opts.json {
                    // the move is in the working tree now: its commit, with the
                    // message beside the index — the note is said once, above
                    println!(
                        "now commit it as one commit (R-177): stage what it wrote, then `git commit -F {}` — the message names the move and carries the note above",
                        docsys::place::shown(&message_path).display()
                    );
                    if !u.last {
                        println!(
                            "\nthen run `docsys upgrade` again: the next move is its own commit"
                        );
                    }
                }
                break;
            }
            ExitCode::SUCCESS
        }
        ("feedback", None) => {
            // a value the command cannot use is a bad invocation, draft or not
            if let Some(extra) = opts.positional.first() {
                eprintln!(
                    "feedback: `{extra}` is no argument of feedback — `docsys feedback --help`"
                );
                return ExitCode::from(2);
            }
            if let Some(k) = opts.kind.as_deref() {
                if !docsys::feedback::TYPES.contains(&k) {
                    eprintln!(
                        "feedback: `{k}` is not one of: {}",
                        docsys::feedback::TYPES.join(", ")
                    );
                    return ExitCode::from(2);
                }
            }
            let rule_line = match opts.rule.as_deref() {
                Some(r) => match docsys::rules::rule_sentence(r) {
                    Some(s) => Some((r, s)),
                    None => {
                        eprintln!("feedback: `{r}` is no rule of the embedded spec");
                        return ExitCode::from(2);
                    }
                },
                None => None,
            };
            if !opts.draft {
                // a draft's own flags would be dropped unsaid
                let shaping = [
                    (opts.kind.is_some(), "--type"),
                    (opts.command.is_some(), "--command"),
                    (opts.out.is_some(), "--out"),
                ];
                if let Some((_, flag)) = shaping.iter().find(|(given, _)| *given) {
                    eprintln!("feedback: {flag} shapes a draft — add --draft");
                    return ExitCode::from(2);
                }
                // the pointer's command: that rule, and how to dispute it
                if let Some((r, sentence)) = rule_line {
                    println!("{sentence}\n");
                    println!("A finding of it that is wrong is a false positive. Draft the issue, the command that reported it included:\n  docsys feedback --draft --rule {r} --command \"docsys …\"\n");
                }
                print!("{}", docsys::feedback::guide());
                return ExitCode::SUCCESS;
            }
            // a rule named means a finding disputed, unless the kind says otherwise
            let kind = opts.kind.clone().unwrap_or_else(|| {
                if opts.rule.is_some() {
                    "false-positive".to_string()
                } else {
                    "bug".to_string()
                }
            });
            let tree = opts
                .root
                .join(".docmeta.yml")
                .is_file()
                .then_some(opts.root.as_path());
            let d = docsys::feedback::Draft {
                kind: &kind,
                rule: opts.rule.as_deref(),
                command: opts.command.as_deref(),
                root: tree,
            };
            match docsys::feedback::draft(&d) {
                Ok(body) => {
                    match &opts.out {
                        Some(p) => {
                            if let Err(e) = std::fs::write(p, &body) {
                                eprintln!("feedback: {e}");
                                return ExitCode::from(2);
                            }
                            eprintln!("draft written to {}", p.display());
                        }
                        None => print!("{body}"),
                    }
                    eprintln!(
                        "fill the TODO parts, read it for private content, then file it at {}",
                        docsys::feedback::new_issue_url(&kind)
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("feedback: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("adopt", None) => {
            let repo = opts.repo.clone().unwrap_or_else(|| PathBuf::from("."));
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            // the workflow flags are refused before anything is written; a tree
            // adopt creates declares this docsys's spec
            let era = if root.join(".docmeta.yml").is_file() {
                docsys::era::Era::at(&root)
            } else {
                docsys::era::Era::of_spec(Some(&format!(
                    "docsys/{}",
                    docsys::rules::spec_version()
                )))
            };
            // a report directory for a report that is not written is a
            // contradiction, refused on a docsys/0.5 tree before anything runs
            if opts.no_report && opts.report_dir.is_some() && era.journal_from_history() {
                eprintln!("adopt: --report-dir names where ADOPTION.md goes, and --no-report writes none — give one");
                return ExitCode::from(2);
            }
            let ci = match docsys::workflow::Ci::from_flags(
                opts.ci_runner.as_deref(),
                opts.ci_install.as_deref(),
                opts.ci_sha256.as_deref(),
                opts.verify_on_approval.as_deref(),
                era,
            ) {
                Ok(ci) => ci,
                Err(e) => {
                    eprintln!("adopt: {e}");
                    return ExitCode::from(2);
                }
            };
            if opts.obsidian {
                match docsys::adopt::obsidian(&root) {
                    Ok(w) if w.is_empty() => println!("obsidian: already configured"),
                    Ok(w) => println!("obsidian: {} written", w.join(", ")),
                    Err(e) => {
                        eprintln!("adopt --obsidian: {e}");
                        return ExitCode::from(2);
                    }
                }
            }
            let place = docsys::adopt::Placement {
                rules_file: opts.rules_file.clone(),
                report_dir: opts.report_dir.clone(),
                no_report: opts.no_report,
                ci,
            };
            match docsys::adopt::run_placed(&repo, &root, &opts.lang, &place) {
                Ok(done) => {
                    for s in &done.summary {
                        println!("{s}");
                    }
                    for text in &done.printed {
                        print!("\n{text}");
                    }
                    if !done.report_path.is_empty() {
                        println!("\nreport + judgment checklist: {}", done.report_path);
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("adopt: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("rules", None) => {
            if opts.procedures {
                match docsys::rules::procedures() {
                    Some(p) => {
                        print!("{p}");
                        ExitCode::SUCCESS
                    }
                    None => {
                        eprintln!("rules: section 14.3 not found in the embedded spec");
                        ExitCode::from(2)
                    }
                }
            } else if opts.agents_md {
                match docsys::rules::check_budget(opts.max_lines) {
                    Ok(_) => {
                        // the tree whose block and preamble these are, found
                        // from where this runs (D-098); a docsys/0.4 tree's
                        // block is the one 0.15.1 wrote (D-118)
                        let root =
                            docsys::place::locate(&docsys::place::cwd_anchor(), &opts.root, None)
                                .root;
                        let v05 = !root.join(".docmeta.yml").is_file()
                            || docsys::era::Era::at(&root).journal_from_history();
                        if let Some(target) = &opts.write {
                            match docsys::rules::write_agents_block_with(
                                target,
                                &docsys::migrate::generated_preamble(&root),
                                v05,
                            ) {
                                Ok(_) => {
                                    println!(
                                        "managed block written to {} (idempotent)",
                                        target.display()
                                    );
                                    return ExitCode::SUCCESS;
                                }
                                Err(e) => {
                                    eprintln!("rules --write: {e}");
                                    return ExitCode::from(2);
                                }
                            }
                        }
                        print!("{}", docsys::rules::agents_md_for(v05));
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("rules: {e}");
                        ExitCode::from(1)
                    }
                }
            } else {
                eprintln!("rules needs --agents-md or --procedures");
                ExitCode::from(2)
            }
        }
        ("seed", Some("plan")) => {
            let (repo, root) = (repo_or_cwd.clone(), opts.root.clone());
            let o = docsys::seed::Options {
                target: opts.target.clone(),
                since: opts.since.clone(),
                memory: opts.memory.clone(),
            };
            match docsys::seed::plan(&repo, &root, &o) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("seed: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("debt", Some("close")) | ("question", Some("close")) => {
            if opts.positional.is_empty() {
                eprintln!("{cmd} close needs the item: its number, or words only it holds");
                return ExitCode::from(2);
            }
            let which = &opts.positional.join(" ");
            let done = if cmd == "debt" {
                docsys::capture::debt_close(&opts.root, which, opts.note.as_deref())
            } else {
                docsys::capture::question_close(&opts.root, which, opts.answer.as_deref())
            };
            match done {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{cmd} close: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("debt", Some("add")) | ("question", Some("add")) => {
            let text = opts.positional.join(" ");
            let done = if cmd == "debt" {
                docsys::capture::debt_add(
                    &opts.root,
                    &text,
                    opts.topic.as_deref(),
                    opts.deferred.as_deref(),
                    opts.repay_when.as_deref(),
                    opts.date.as_deref(),
                )
            } else {
                docsys::capture::question_add(
                    &opts.root,
                    &text,
                    opts.topic.as_deref(),
                    opts.context.as_deref(),
                    opts.date.as_deref(),
                )
            };
            match done {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{cmd} add: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("ledger", Some("fix")) => match docsys::capture::ledger_fix(&opts.root) {
            Ok(msg) => {
                println!("{msg}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("ledger fix: {e}");
                ExitCode::from(1)
            }
        },
        ("journal", None) if !docsys::era::Era::at(&opts.root).journal_from_history() => {
            // a docsys/0.4 tree's journal is its own file (D-118)
            print!(
                "{}",
                docsys::journal::render_files(&opts.root, opts.since.as_deref())
            );
            ExitCode::SUCCESS
        }
        ("journal", None) => {
            let repo = docsys::repo_of(&opts.root);
            print!(
                "{}",
                docsys::journal::render(repo.as_deref(), &opts.root, opts.since.as_deref())
            );
            ExitCode::SUCCESS
        }
        ("journal", Some("add")) => {
            let text = opts.positional.join(" ");
            match docsys::capture::journal_add(
                &opts.root,
                &text,
                opts.title.as_deref(),
                opts.date.as_deref(),
                opts.link.as_deref(),
            ) {
                Ok(msg) if docsys::era::Era::at(&opts.root).journal_from_history() => {
                    if msg.starts_with("Docs:") {
                        eprintln!("journal: the entry is the commit — end its message with this line (D-125)");
                    } else {
                        eprintln!(
                            "journal: the entry is the commit — commit with this message (D-125)"
                        );
                    }
                    print!("{msg}");
                    ExitCode::SUCCESS
                }
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("journal add: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("page", Some("new")) => {
            let (Some(kind), Some(id)) = (opts.positional.first(), opts.positional.get(1)) else {
                eprintln!("page new needs a kind and an id: `docsys page new <feature|postmortem|research|reference|howto|explanation|tutorial> <id>`");
                return ExitCode::from(2);
            };
            match docsys::capture::page_new(
                &opts.root,
                kind,
                id,
                opts.title.as_deref(),
                opts.unverified,
            ) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("page new: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("backlinks", None) | ("mentions", None) | ("graph", None) => {
            let tree = match docsys::tree::DocTree::load(&opts.root) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("{cmd}: {e}");
                    return ExitCode::from(2);
                }
            };
            // the repository: a given one, else the tree's own, as help says
            let repo = opts
                .repo
                .clone()
                .or_else(|| here.as_ref().and_then(|p| p.repo.clone()));
            let repo = repo.as_deref();
            let result = match cmd {
                "backlinks" => match opts.positional.first() {
                    // a code file, not a page: the pages that describe it
                    Some(w) if !docsys::graph::is_page(&tree, w) => {
                        match code_file(w, repo_or_cwd.as_path()) {
                            Some(rel) => Ok(docsys::graph::describing(&tree, &rel)),
                            None => docsys::graph::backlinks(&tree, repo, w),
                        }
                    }
                    Some(w) => docsys::graph::backlinks(&tree, repo, w),
                    None => Err("backlinks needs a page path or id, or a code file".to_string()),
                },
                "mentions" => {
                    docsys::graph::mentions(&tree, opts.positional.first().map(String::as_str))
                }
                _ => docsys::graph::render(&tree, repo, opts.format.as_deref().unwrap_or("dot")),
            };
            match result {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{cmd}: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("seed", Some("gaps")) => {
            let (repo, root) = (repo_or_cwd.clone(), opts.root.clone());
            let o = docsys::seed::Options {
                target: None,
                since: opts.since.clone(),
                memory: None,
            };
            match docsys::seed::gaps_json(&repo, &root, &o) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("seed: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("seed", Some("apply")) => {
            let (repo, root) = (repo_or_cwd.clone(), opts.root.clone());
            let Some(plan) = opts.plan.clone() else {
                eprintln!("seed apply needs --plan <file>");
                return ExitCode::from(2);
            };
            match docsys::seed::apply(&repo, &root, &plan, opts.force) {
                Ok(done) => {
                    for d in done {
                        println!("{d}");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("seed apply: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("hook", Some(event)) => {
            // The payload comes on stdin from the agent harness. `stop` needs
            // none, and a human at a terminal must not be left waiting for EOF.
            let mut payload = String::new();
            // `stop` reads it only when the relay says so (`--stdin`): a script
            // calling the hook by hand must not hang on an open pipe.
            let wants = event != "stop" || opts.stdin;
            if wants && !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut payload);
            }
            // The tree is found from the payload — the edited file, the
            // session's directory — not from wherever the harness started the
            // relay (D-098).
            let (file, cwd) = docsys::hook::payload_places(&payload);
            let here = docsys::place::locate(
                &docsys::place::hook_anchors(file.as_deref(), cwd.as_deref()),
                &opts.root,
                opts.repo.as_deref(),
            );
            let root = here.root;
            let repo = here
                .repo
                .or_else(|| opts.repo.clone())
                .unwrap_or_else(|| PathBuf::from("."));
            let reply = match event {
                "pre-tool-use" => docsys::hook::pre_tool_use(
                    &repo,
                    &root,
                    &payload,
                    std::env::var_os("DOCSYS_SKIP").is_some_and(|v| !v.is_empty()),
                ),
                "stop" => docsys::hook::stop(&repo, &root, &payload),
                "post-tool-use" => {
                    docsys::hook::post_tool_use(&repo, &root, &payload, &migrate::today())
                }
                "user-prompt-submit" => docsys::hook::user_prompt_submit(&payload, &root),
                other => {
                    eprintln!("hook: unknown event `{other}` (pre-tool-use | stop | post-tool-use | user-prompt-submit)");
                    return ExitCode::from(2);
                }
            };
            print!("{}", reply.stdout);
            eprint!("{}", reply.stderr);
            ExitCode::from(reply.code)
        }
        ("lookup", None) => {
            if opts.positional.is_empty() {
                eprintln!("lookup needs at least one word");
                return ExitCode::from(2);
            }
            match docsys::lookup::lookup(&opts.root, &opts.positional) {
                Ok(hits) => {
                    if opts.json {
                        print!("{}", docsys::lookup::render_json(&hits));
                    } else {
                        print!("{}", docsys::lookup::render(&hits, &opts.positional));
                    }
                    if hits.is_empty() {
                        ExitCode::from(1)
                    } else {
                        ExitCode::SUCCESS
                    }
                }
                Err(e) => {
                    eprintln!("lookup: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("consume", Some("add")) => match opts.positional.first() {
            Some(target) => match docsys::consume::add(&opts.root, target, opts.as_ns.as_deref()) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("consume add: {e}");
                    ExitCode::from(2)
                }
            },
            None => {
                eprintln!("consume add needs <path|git-url>[#subdir]");
                ExitCode::from(2)
            }
        },
        ("consume", Some("discover")) => {
            let dir = opts
                .positional
                .first()
                .map_or_else(|| PathBuf::from("."), PathBuf::from);
            match docsys::consume::discover(&opts.root, &dir) {
                Ok(found) if found.is_empty() => {
                    println!("no docsys tree one level under {}", dir.display());
                    ExitCode::SUCCESS
                }
                Ok(found) => {
                    for c in &found {
                        let action = if c.already {
                            "already consumed".to_string()
                        } else {
                            format!(
                                "docsys consume add {} --root {}",
                                c.path.display(),
                                opts.root.display()
                            )
                        };
                        println!("{}\t{}\t{}\t{action}", c.ns, c.profile, c.path.display());
                    }
                    println!("-- {} candidate(s); nothing written", found.len());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("consume discover: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("inbox", Some("add")) => {
            let (Some(source), Some(id)) = (opts.source.clone(), opts.source_id.clone()) else {
                eprintln!("inbox add needs --source <name> and --id <item id at the source>");
                return ExitCode::from(2);
            };
            // the body: a file, `-` for stdin, or nothing
            let body = match opts.positional.first().map(String::as_str) {
                Some("-") => {
                    let mut s = String::new();
                    let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut s);
                    s
                }
                Some(path) => match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("inbox add: cannot read `{path}`: {e}");
                        return ExitCode::from(2);
                    }
                },
                None => String::new(),
            };
            let p = docsys::inbox::Provenance {
                source,
                title: opts.title.clone().unwrap_or_else(|| id.clone()),
                source_id: id,
                url: opts.url.clone(),
                date: opts.date.clone().unwrap_or_else(migrate::today),
            };
            match docsys::inbox::add(&opts.root, &p, &body) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("inbox add: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("inbox", Some("pull")) => {
            let Some(repo) = opts.positional.first() else {
                eprintln!("inbox pull needs <repo>");
                return ExitCode::from(2);
            };
            let since = opts.since.clone().unwrap_or_else(|| "7.days".to_string());
            match docsys::inbox::pull_git(
                &opts.root,
                &PathBuf::from(repo),
                &since,
                opts.as_ns.as_deref(),
                opts.limit,
                opts.all,
            ) {
                Ok(lines) => {
                    for l in &lines {
                        println!("{l}");
                    }
                    let new = lines.iter().filter(|l| l.starts_with("captured:")).count();
                    println!(
                        "-- {new} new record(s), {} already captured",
                        lines.len() - new
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("inbox pull: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("assistant", None) => {
            let since = opts.since.clone().unwrap_or_else(|| "30.days".to_string());
            let limit = Some(opts.limit.unwrap_or(3));
            match docsys::assistant::run(&opts.root, &opts.projects, &opts.domains, &since, limit) {
                Ok(done) => {
                    for s in &done.steps {
                        println!("{s}");
                    }
                    println!();
                    match docsys::status::status(&opts.root, docsys::repo_of(&opts.root).as_deref())
                    {
                        Ok(s) => print!("{}", docsys::status::render(&s, &opts.root)),
                        Err(e) => eprintln!("status: {e}"),
                    }
                    println!(
                        "
next: review, `git add -A && git commit`, then open an agent session here."
                    );
                    println!("  the first session proposes the assistant's character (name, address, tone, languages,");
                    println!("  never-do) and asks you to confirm — in your language; the answers land in AGENTS.md.");
                    println!("  then try:");
                    println!("  \"how does <project> handle <thing>?\"      lookup — cites @namespace/id");
                    println!("  \"study what my projects say about X and write it up\"   a page whose sources are theirs");
                    println!("  \"process my inbox\"   the commit records, distilled or left with a reason");
                    println!("  \"audit the wiki\"     in another session");
                    println!("  \"my morning briefing\"   from `docsys status`");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("assistant: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("verify", None) if opts.approval.is_some() => {
            // the approval job: the line a maintainer's approval adds to the
            // pull request's description (D-126)
            match docsys::verify::approval_line(&opts.root, opts.approval.as_deref().unwrap_or(""))
            {
                Ok(Some(line)) => {
                    println!("{line}");
                    ExitCode::SUCCESS
                }
                Ok(None) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("verify: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("verify", None) if opts.range.is_some() => {
            match docsys::verify::verify_range(
                &opts.root,
                opts.range.as_deref().unwrap_or(""),
                opts.by.as_deref(),
                opts.from_trailers,
                opts.commit,
            ) {
                Ok(docsys::verify::Range::NotAMaintainer(login)) => {
                    println!("skipped: @{login} is not a declared maintainer");
                    ExitCode::SUCCESS
                }
                Ok(docsys::verify::Range::Pages(done)) => {
                    for v in &done {
                        if v.notes.iter().any(|n| n.starts_with("skipped")) {
                            println!("skipped: {} — {}", v.page, v.notes.join("; "));
                        } else if v.notes.iter().any(|n| n.starts_with("already")) {
                            println!("already verified: {}", v.page);
                        } else {
                            println!(
                                "verified: {} by {} at {}{}",
                                v.page,
                                v.by,
                                v.rev,
                                if v.committed { " (committed)" } else { "" }
                            );
                        }
                    }
                    if done.is_empty() {
                        println!("no page with a verification field in that range");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("verify: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("verify", None) if opts.show => match opts.positional.first() {
            Some(page) => match docsys::verify::show(&opts.root, page) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("verify: {e}");
                    ExitCode::from(2)
                }
            },
            None => {
                eprintln!("verify --show needs <page-id|page-path>");
                ExitCode::from(2)
            }
        },
        ("verify", None) => match opts.positional.first() {
            Some(page) => match docsys::verify::verify(
                &opts.root,
                page,
                opts.by.as_deref(),
                opts.commit,
                opts.revoke,
            ) {
                Ok(done) => {
                    if opts.revoke {
                        println!("unverified: {}", done.page);
                    } else {
                        println!("verified: {} by {} at {}", done.page, done.by, done.rev);
                    }
                    for n in &done.notes {
                        println!("{n}");
                    }
                    if !opts.revoke
                        && !done.committed
                        && !done.notes.iter().any(|n| n.starts_with("already"))
                    {
                        println!(
                            "now commit it as yourself (the record must be your own commit, R-208): git commit -m \"docs: {} verified\" -- <the page>   (or run again with --commit)",
                            done.page.trim_end_matches(".md")
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("verify: {e}");
                    ExitCode::from(2)
                }
            },
            None => {
                eprintln!("verify needs <page-id|page-path> [--by <handle>] [--commit] [--revoke]");
                ExitCode::from(2)
            }
        },
        ("raw", Some("move")) => match (opts.positional.first(), opts.positional.get(1)) {
            (Some(record), Some(domain)) => {
                match docsys::relocate::raw_move(&opts.root, record, domain) {
                    Ok(done) => {
                        println!("moved: {} -> {}", done.from, done.to);
                        for (page, n) in &done.rewritten {
                            let unit = if *n == 1 { "entry" } else { "entries" };
                            println!("rewrote: {page} ({n} {unit})");
                        }
                        if done.rewritten.is_empty() {
                            println!("no page cited it");
                        }
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("raw move: {e}");
                        ExitCode::from(2)
                    }
                }
            }
            _ => {
                eprintln!("raw move needs <record> <domain>");
                ExitCode::from(2)
            }
        },
        ("forget", None) => match (
            opts.positional.first(),
            opts.note.clone().or_else(|| opts.title.clone()),
        ) {
            (Some(target), Some(reason)) => {
                match docsys::forget::forget(&opts.root, target, &reason) {
                    Ok(done) => {
                        for step in &done.steps {
                            println!("{step}");
                        }
                        println!("forgotten. The bytes stay where an audit finds them; erasing history is `git filter-repo`, a person's act.");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("forget: {e}");
                        ExitCode::from(2)
                    }
                }
            }
            _ => {
                eprintln!("forget needs <page-id|page-path|record-path> and --reason <text>");
                ExitCode::from(2)
            }
        },
        ("status", None) => {
            let repo = here.as_ref().and_then(|p| p.repo.clone());
            match docsys::status::status(&opts.root, repo.as_deref()) {
                Ok(s) => {
                    if opts.json {
                        print!("{}", docsys::status::render_json(&s));
                    } else {
                        print!("{}", docsys::status::render(&s, &opts.root));
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("status: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("compile", None) => match opts.positional.first() {
            Some(page) => match docsys::compile::compile(&opts.root, &opts.dir, page, opts.force) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("compile: {e}");
                    ExitCode::from(2)
                }
            },
            None => {
                eprintln!("compile needs <howto> (an id or a root-relative path)");
                ExitCode::from(2)
            }
        },
        ("pin", None) => {
            let (repo, root) = (repo_or_cwd.clone(), opts.root.clone());
            let result = if opts.gc {
                docsys::fresh::gc(&root, &repo).map(|done| {
                    if done.is_empty() {
                        "pin --gc: nothing to remove".to_string()
                    } else {
                        done.join("\n")
                    }
                })
            } else if opts.refresh {
                match opts.positional.first() {
                    Some(page) => docsys::fresh::refresh(&root, &repo, page),
                    None => Err("pin --refresh needs <page>".to_string()),
                }
            } else {
                match (opts.positional.first(), opts.positional.get(1)) {
                    (Some(page), Some(path)) => docsys::fresh::pin_block(
                        &root,
                        &repo,
                        page,
                        path,
                        opts.symbol.as_deref(),
                        opts.block,
                    ),
                    _ => Err(
                        "pin needs <page> <path> [--symbol <s>] [--block <n>], --refresh <page>, or --gc"
                            .to_string(),
                    ),
                }
            };
            match result {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("pin: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("gate", None) if opts.message.is_some() => {
            // the `commit-msg` hook: the message that will carry the change
            // set (D-125)
            let (repo, root) = (repo_or_cwd.clone(), opts.root.clone());
            let text = opts
                .message
                .as_ref()
                .and_then(|m| std::fs::read_to_string(m).ok())
                .unwrap_or_default();
            let v = docsys::gate::message(&repo, &root, &text);
            for r in &v.reports {
                eprintln!("{r}");
            }
            match v.refusal {
                Some(r) => {
                    eprintln!("{r}");
                    ExitCode::from(1)
                }
                None => ExitCode::SUCCESS,
            }
        }
        ("gate", None) => {
            let (repo, root) = (repo_or_cwd.clone(), opts.root.clone());
            let result = match &opts.range {
                Some(r) => docsys::gate::run_range(&repo, &root, r),
                None => docsys::gate::run(&repo, &root),
            };
            match result {
                Ok((g, report)) => {
                    for f in &report.findings {
                        println!(
                            "{} {} {} [{}] {}",
                            f.severity.tag(),
                            f.rule,
                            f.file,
                            f.subject,
                            f.message
                        );
                    }
                    if !g.plan_files.is_empty() {
                        println!("{}", docsys::say::gate_seed_plan(&g.plan_files));
                    }
                    let require =
                        docsys::hook::commit_policy(&root) == docsys::hook::CommitPolicy::Require;
                    // docsys/0.5: the message is the record, and only the
                    // commit-msg gate and a range can read it (D-125)
                    let history_journal = docsys::era::Era::at(&root).journal_from_history();
                    let by_message = history_journal
                        && opts
                            .range
                            .as_deref()
                            .is_some_and(|r| docsys::gate::range_has_docs(&repo, r));
                    let undocumented = !g.code.is_empty() && g.docs == 0 && !by_message;
                    if undocumented && require && opts.skipped {
                        // the git hook, bypassed with DOCSYS_SKIP=1: the bypass leaves a debt item (D-093)
                        match docsys::hook::record_undocumented_commit(
                            &root,
                            &g.code,
                            &migrate::today(),
                        ) {
                            Ok(file) => println!("{}", docsys::say::gate_bypassed(&file)),
                            Err(e) => eprintln!("gate: could not record the bypass: {e}"),
                        }
                    }
                    // under require on docsys/0.5 the commit-msg gate, which
                    // reads the message, is the one that speaks (D-125)
                    let deferred =
                        history_journal && require && !opts.skipped && opts.range.is_none();
                    if undocumented && !deferred {
                        println!("{}", docsys::say::gate_undocumented(g.scope, &g.code));
                        if require && !opts.skipped && !history_journal {
                            println!(
                                "{}",
                                docsys::say::era_text(&root, docsys::say::GATE_REQUIRE_015)
                            );
                        }
                    }
                    println!(
                        "-- {} error(s), {} warning(s)",
                        g.lint_errors, g.lint_warnings
                    );
                    if docsys::era::Era::at(&root).finding_pointers() {
                        for p in docsys::feedback::pointers_once(
                            &root,
                            report.findings.iter().map(|f| f.rule.0),
                        ) {
                            println!("{p}");
                        }
                    }
                    // Over a range there is nobody to ask once: code without
                    // documentation fails the check, as CI must.
                    let unanswered = (opts.range.is_some()
                        || (require && !opts.skipped && !history_journal))
                        && undocumented;
                    if g.lint_errors > 0 || unanswered || !g.plan_files.is_empty() {
                        ExitCode::from(1)
                    } else {
                        ExitCode::SUCCESS
                    }
                }
                Err(e) => {
                    eprintln!("gate: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("doctor", None) => {
            let (repo, root) = (repo_or_cwd.clone(), opts.root.clone());
            // the agent layer lives at the repository, wherever the command runs
            let dir = if opts.dir.is_relative() && repo != std::path::Path::new(".") {
                repo.join(&opts.dir)
            } else {
                opts.dir.clone()
            };
            let d = docsys::doctor::run(&repo, &root, &dir);
            for l in &d.lines {
                println!("{l}");
            }
            if d.failed > 0 {
                println!(
                    "-- {} check(s) FAILED — the pipeline is not alive",
                    d.failed
                );
                ExitCode::from(1)
            } else {
                println!("-- pipeline alive");
                ExitCode::SUCCESS
            }
        }
        ("fetch", None) => match docsys::export::fetch(&opts.root) {
            Ok(summary) => {
                for s in &summary {
                    println!("{s}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("fetch: {e}");
                ExitCode::from(2)
            }
        },
        ("export", Some("manifest")) => match docsys::export::manifest(&opts.root) {
            Ok(text) => match &opts.out {
                Some(path) => match docsys::export::write_if_changed(path, &text) {
                    Ok(true) => {
                        eprintln!("manifest -> {}", path.display());
                        ExitCode::SUCCESS
                    }
                    Ok(false) => {
                        eprintln!("unchanged — {} left untouched", path.display());
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("export manifest: {e}");
                        ExitCode::from(2)
                    }
                },
                None => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
            },
            Err(e) => {
                eprintln!("export manifest: {e}");
                ExitCode::from(2)
            }
        },
        ("export", Some("plan")) => {
            match docsys::export::plan(&opts.root, opts.audience.as_deref()) {
                Ok(p) => {
                    print!("{p}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("export plan: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("export", Some("product")) => {
            let Some(map) = opts.positional.first() else {
                eprintln!("export product needs a <map> argument");
                return ExitCode::from(2);
            };
            let want_lang = opts.lang_explicit.then_some(opts.lang.as_str());
            match docsys::export::product(
                &opts.root,
                std::path::Path::new(map),
                want_lang,
                opts.audience.as_deref(),
            ) {
                Ok(done) => emit_export(&done, opts.out.as_deref()),
                Err(e) => {
                    eprintln!("export product: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("export", Some("feature")) => {
            if opts.positional.is_empty() {
                eprintln!("export feature needs at least one <id>");
                return ExitCode::from(2);
            }
            let want_lang = opts.lang_explicit.then_some(opts.lang.as_str());
            match docsys::export::feature(
                &opts.root,
                &opts.positional,
                opts.follow,
                opts.title.clone(),
                want_lang,
                opts.audience.as_deref(),
            ) {
                Ok(done) => emit_export(&done, opts.out.as_deref()),
                Err(e) => {
                    eprintln!("export feature: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("graduate", Some("plan")) => {
            let Some(src) = opts.positional.first() else {
                eprintln!("graduate plan needs a <work-file> argument");
                return ExitCode::from(2);
            };
            match docsys::graduate::plan(&opts.root, src) {
                Ok(p) => {
                    print!("{p}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("graduate plan: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("graduate", Some("apply")) => {
            let Some(plan_path) = &opts.plan else {
                eprintln!("graduate apply needs --plan <file>");
                return ExitCode::from(2);
            };
            let plan = match std::fs::read_to_string(plan_path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("plan: {e}");
                    return ExitCode::from(2);
                }
            };
            let done = match &opts.confirmed {
                Some(who) => docsys::graduate::apply_confirmed(&opts.root, &plan, opts.force, who),
                None => docsys::graduate::apply(&opts.root, &plan, opts.force),
            };
            match done {
                Ok(done) => {
                    println!(
                        "moved {} block(s) · linked {} · destinations touched: {}",
                        done.moved,
                        done.linked,
                        done.dest_files.join(", ")
                    );
                    if let (Some(removed), Some(message)) = (&done.removed, &done.message) {
                        println!("removed {removed} — history keeps it");
                        println!("commit the change with this message (D-127):\n{message}");
                    }
                    println!("-- running lint --");
                    run_lint(&Opts {
                        json: false,
                        ..opts
                    })
                }
                Err(e) => {
                    eprintln!("graduate apply: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("agents", None) if opts.procedures => {
            eprintln!("agents: the procedures are `docsys rules --procedures`; `docsys agents --report` lists the existing layer");
            ExitCode::from(2)
        }
        ("agents", None) if opts.report => {
            // --report: mechanical inventory of the existing layer (D-026).
            println!(
                "existing agent layer under {}:",
                docsys::place::shown(&opts.dir).display()
            );
            let repo = opts
                .dir
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            let exact = docsys::era::Era::at(&repo.join(agents_root(&opts))).journal_from_history();
            for line in docsys::agents::adoption_report(&opts.dir, exact) {
                println!("  {line}");
            }
            println!("\ndelegation is judgment: keep the owner's prose, repoint the");
            println!("mechanical calls to docsys where they duplicate a command.");
            ExitCode::SUCCESS
        }
        ("agents", None) if opts.kb => {
            // The base is named from the repository's top, wherever this runs
            // (D-098): a given root there, else the repository's one tree — a
            // knowledge base is usually its own repository.
            let repo = opts
                .dir
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            let base = if opts.root.is_absolute() {
                opts.root.clone()
            } else if opts.root.as_path() != std::path::Path::new("docs")
                || repo.join(&opts.root).join(".docmeta.yml").is_file()
            {
                repo.join(&opts.root)
            } else {
                docsys::git::toplevel(repo)
                    .and_then(|top| docsys::place::only_tree(&top))
                    .unwrap_or_else(|| repo.to_path_buf())
            };
            // the installer serves the tree it names and no other profile's
            match docsys::agents::install_kb(&opts.dir, &base, opts.force) {
                Ok(done) => {
                    for p in &done.paths {
                        println!("wrote   {}", docsys::place::shown(p).display());
                    }
                    for f in &done.skipped {
                        let p = if f == "AGENTS.md" {
                            base.join(f)
                        } else {
                            opts.dir.join(f)
                        };
                        println!(
                            "skipped {} (exists; --force to overwrite)",
                            docsys::place::shown(&p).display()
                        );
                    }
                    for n in &done.notes {
                        println!("{n}");
                    }
                    println!("\nthe base's four organs: capture · ingest · audit · lookup.");
                    println!(
                        "declare your subjects in .docmeta.yml `domains:` and start capturing."
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("agents --kb: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("agents", None) => match docsys::agents::install_with_preamble(
            &opts.dir,
            opts.force,
            "",
            &agents_root(&opts).to_string_lossy(),
        ) {
            Ok(done) => {
                let dir = docsys::place::shown(&opts.dir);
                for f in &done.written {
                    println!("wrote   {}/{f}", dir.display());
                }
                for n in &done.notes {
                    println!("{n}");
                }
                for f in &done.skipped {
                    println!(
                        "skipped {}/{f} (exists; --force to overwrite)",
                        dir.display()
                    );
                }
                // what is left to wire by hand, and only that
                let repo = opts
                    .dir
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(std::path::Path::new("."));
                // a docsys/0.5 tree has no post-edit relay (D-126)
                let post_edit = !docsys::era::Era::at(&repo.join(agents_root(&opts)))
                    .verification_from_history();
                let wired =
                    docsys::agents::settings_wired(&opts.dir.join("settings.json"), post_edit);
                let holder = docsys::adopt::rules_block_holder(repo);
                if let (true, Some(file)) = (wired, &holder) {
                    println!(
                        "the agent layer is wired: settings.json runs every relay, and {} holds the rules block",
                        file.strip_prefix(repo).unwrap_or(file).display()
                    );
                    return ExitCode::SUCCESS;
                }
                if !wired {
                    println!("\n-- merge into .claude/settings.json by hand (protected file): --");
                    println!("{}", docsys::agents::settings_snippet(post_edit));
                }
                if holder.is_none() {
                    println!("-- and add the generated block to AGENTS.md: --");
                    println!("   docsys rules --agents-md >> AGENTS.md   # review the diff first");
                }
                println!("\ntip: `docsys adopt` does all of this in one pass — assets,");
                println!("settings.json (when absent), AGENTS.md block, git gate, report.");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("agents: {e}");
                ExitCode::from(2)
            }
        },
        ("refs", None) => {
            // the repository is the tree's own, a given --repo its top level (D-098)
            let Some(repo) = here
                .as_ref()
                .and_then(|p| p.repo.clone())
                .or_else(|| opts.repo.clone())
            else {
                eprintln!("refs: the tree is not inside a repository — --repo <dir> names one");
                return ExitCode::from(2);
            };
            let repo = &repo;
            // The root was found from the repo (D-098, anchored as D-027 asks): a
            // bare `docs` next to a walk that yields `./docs/...` fails the
            // inside-the-tree prefix test and the docs tree gets scanned as code.
            let root = opts.root.clone();
            let tree = match docsys::tree::DocTree::load(&root) {
                Ok(t) if t.docmeta_present => t,
                Ok(_) => {
                    eprintln!("refs: `{}` has no .docmeta.yml", root.display());
                    return ExitCode::from(2);
                }
                Err(e) => {
                    eprintln!("refs: {e}");
                    return ExitCode::from(2);
                }
            };
            let report = docsys::refs::run(repo, &tree);
            if opts.json {
                print!("{}", to_json(&report));
            } else {
                for f in &report.findings {
                    println!(
                        "{} {} {} [{}] {}",
                        f.severity.tag(),
                        f.rule,
                        f.file,
                        f.subject,
                        f.message
                    );
                }
                let errors = report
                    .findings
                    .iter()
                    .filter(|f| f.severity == docsys::model::Severity::Error)
                    .count();
                let units: usize = report.inspected.values().sum();
                println!(
                    "-- {errors} error(s), {} warning(s); {units} unit(s) inspected",
                    report.findings.len() - errors
                );
                if docsys::era::Era::at(&root).finding_pointers() {
                    for p in docsys::feedback::pointers_once(
                        &root,
                        report.findings.iter().map(|f| f.rule.0),
                    ) {
                        println!("{p}");
                    }
                }
            }
            if report
                .findings
                .iter()
                .any(|f| f.severity == docsys::model::Severity::Error)
            {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        ("init", None) => match migrate::init_profile(
            &opts.root,
            &opts.lang,
            opts.profile.as_deref().unwrap_or("project"),
        ) {
            Ok(()) => {
                println!("initialized `{}`", opts.root.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("init: {e}");
                ExitCode::from(2)
            }
        },
        ("migrate", Some("inventory")) => {
            let (root, repo) = migrate_paths(&opts, root_given);
            match migrate::inventory(&root) {
                Ok(plan) => {
                    print!("{plan}");
                    if let Some(repo) = repo {
                        println!("# -- inbound references from the repo (will need rewriting) --");
                        for (file, hits) in migrate::inbound_report(&repo, &root) {
                            println!("# inbound: {file} · {hits} reference(s)");
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("inventory: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("migrate", Some("apply")) => {
            let Some(plan_path) = &opts.plan else {
                eprintln!("migrate apply needs --plan <file>");
                return ExitCode::from(2);
            };
            let plan = match std::fs::read_to_string(plan_path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("plan: {e}");
                    return ExitCode::from(2);
                }
            };
            let (root, repo) = migrate_paths(&opts, root_given);
            match migrate::apply(&root, &plan, &opts.lang, repo.as_deref()) {
                Ok(done) => {
                    println!(
                        "moved {} · kept {} · archived {} · links rewritten {}",
                        done.moved, done.kept, done.archived, done.links_rewritten
                    );
                    for (file, n) in &done.repo_rewrites {
                        println!("repo rewrite: {file} · {n} reference(s)");
                    }
                    for risk in &done.repo_risks {
                        println!("RISK unmapped inbound: {risk}");
                    }
                    println!("-- running lint on the migrated tree --");
                    run_lint(&Opts {
                        json: false,
                        root,
                        ..opts
                    })
                }
                Err(e) => {
                    eprintln!("apply: {e}");
                    ExitCode::from(2)
                }
            }
        }
        _ => {
            let r = docsys::help::unknown(&std::iter::once(cmd).chain(sub).collect::<Vec<_>>());
            eprintln!("{}", r.line);
            eprint!("{}", r.entries);
            ExitCode::from(2)
        }
    }
}
