//! `docsys hook <event>` — the agent-hook logic, in the binary.
//!
//! Field lesson, one day long: every hook defect found in the pilot lived in
//! the shell relay — a JSON payload read with `grep`, a heredoc body mistaken
//! for a command, a path quoted by git, a `\t` never opened. The shell cannot
//! be unit-tested and has no JSON parser; the binary has both. So the scripts
//! `docsys agents` installs are one-line relays, and the decisions are made
//! here, where a table of payloads can pin them (D-051).

use crate::gate;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ───────────────────────────── JSON (the subset a hook payload needs)

/// A JSON value, enough to reach a string field inside nested objects.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// `get(&["tool_input", "command"])` — the string at that path, if any.
    pub fn string_at(&self, path: &[&str]) -> Option<&str> {
        let mut cur = self;
        for key in path {
            match cur {
                Json::Obj(fields) => cur = &fields.iter().find(|(k, _)| k == key)?.1,
                _ => return None,
            }
        }
        match cur {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The value as JSON text — two-space indent, one field per line, a
    /// trailing newline: the shape a person reads back in a settings file.
    /// Key order is the value's own (D-086: a merged file keeps its order).
    pub fn render(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        let indent = |out: &mut String, d: usize| out.push_str(&"  ".repeat(d));
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => out.push_str(n),
            Json::Str(s) => push_json_string(out, s),
            Json::Arr(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    indent(out, depth + 1);
                    item.write(out, depth + 1);
                    if i + 1 < items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                indent(out, depth);
                out.push(']');
            }
            Json::Obj(fields) => {
                if fields.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push_str("{\n");
                for (i, (k, v)) in fields.iter().enumerate() {
                    indent(out, depth + 1);
                    push_json_string(out, k);
                    out.push_str(": ");
                    v.write(out, depth + 1);
                    if i + 1 < fields.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                indent(out, depth);
                out.push('}');
            }
        }
    }
}

fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parse a JSON document. Malformed input is `None` — a hook must never
/// guess what a payload meant.
pub fn parse_json(text: &str) -> Option<Json> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.value()?;
    p.ws();
    (p.i == p.s.len()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, b: u8) -> Option<()> {
        (self.peek() == Some(b)).then(|| self.i += 1)
    }
    fn value(&mut self) -> Option<Json> {
        match self.peek()? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(Json::Str),
            b't' => self.literal("true", Json::Bool(true)),
            b'f' => self.literal("false", Json::Bool(false)),
            b'n' => self.literal("null", Json::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }
    fn literal(&mut self, word: &str, v: Json) -> Option<Json> {
        let end = self.i + word.len();
        (self.s.get(self.i..end)? == word.as_bytes()).then(|| {
            self.i = end;
            v
        })
    }
    fn number(&mut self) -> Option<Json> {
        let start = self.i;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.i += 1;
        }
        let raw = self.s.get(start..self.i)?;
        (self.i > start).then(|| Json::Num(String::from_utf8_lossy(raw).into()))
    }
    fn object(&mut self) -> Option<Json> {
        self.eat(b'{')?;
        let mut fields = Vec::new();
        self.ws();
        if self.eat(b'}').is_some() {
            return Some(Json::Obj(fields));
        }
        loop {
            self.ws();
            let k = self.string()?;
            self.ws();
            self.eat(b':')?;
            self.ws();
            let v = self.value()?;
            fields.push((k, v));
            self.ws();
            if self.eat(b',').is_some() {
                continue;
            }
            self.eat(b'}')?;
            return Some(Json::Obj(fields));
        }
    }
    fn array(&mut self) -> Option<Json> {
        self.eat(b'[')?;
        let mut items = Vec::new();
        self.ws();
        if self.eat(b']').is_some() {
            return Some(Json::Arr(items));
        }
        loop {
            self.ws();
            items.push(self.value()?);
            self.ws();
            if self.eat(b',').is_some() {
                continue;
            }
            self.eat(b']')?;
            return Some(Json::Arr(items));
        }
    }
    fn hex4(&mut self) -> Option<u32> {
        let h = std::str::from_utf8(self.s.get(self.i..self.i + 4)?).ok()?;
        let v = u32::from_str_radix(h, 16).ok()?;
        self.i += 4;
        Some(v)
    }
    fn string(&mut self) -> Option<String> {
        self.eat(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let b = self.peek()?;
            self.i += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let e = self.peek()?;
                    self.i += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) {
                                // surrogate pair
                                self.eat(b'\\')?;
                                self.eat(b'u')?;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return None;
                                }
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                            }
                            let ch = char::from_u32(cp)?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return None,
                    }
                }
                _ => out.push(b),
            }
        }
        String::from_utf8(out).ok()
    }
}

// ───────────────────────────── the command a Bash call runs

/// The command with every heredoc BODY removed: a rule text or a commit
/// message quoting the words `git commit` is not a command. The heredoc line
/// itself stays — `git commit -F - <<'MSG'` is a commit (D-050).
pub fn command_lines(cmd: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut term: Option<(String, bool)> = None; // (terminator, `<<-` strips tabs)
    for line in cmd.lines() {
        if let Some((t, dash)) = &term {
            let l = if *dash {
                line.trim_start_matches('\t')
            } else {
                line
            };
            if l == t {
                term = None;
            }
            continue;
        }
        out.push(line);
        if let Some(pos) = line.find("<<") {
            let after = &line[pos + 2..];
            let (dash, after) = match after.strip_prefix('-') {
                Some(a) => (true, a),
                None => (false, after),
            };
            let after = after.trim_start_matches([' ', '\t']);
            let after = after.trim_start_matches(['"', '\'']);
            let word: String = after
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !word.is_empty() && !word.starts_with(|c: char| c.is_ascii_digit()) {
                term = Some((word, dash));
            }
        }
    }
    out
}

/// Does this Bash call commit? `git commit` on a command line — quoted on a
/// command line still counts (cheap to stop at, one re-run to clear).
pub fn is_git_commit(cmd: &str) -> bool {
    command_lines(cmd).iter().any(|l| l.contains("git commit"))
}

pub fn has_git_add(cmd: &str) -> bool {
    command_lines(cmd).iter().any(|l| l.contains("git add"))
}

// ───────────────────────────── git, read unquoted

fn git_lines(repo: &Path, args: &[&str]) -> Vec<String> {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "core.quotePath=false"])
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn git_first(repo: &Path, args: &[&str]) -> Option<String> {
    git_lines(repo, args).into_iter().next()
}

/// Paths from `git status --porcelain`: the two status columns dropped, a
/// rename reported by its NEW path.
pub fn porcelain_paths(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|l| l.len() > 3)
        .map(|l| {
            let p = &l[3..];
            match p.rsplit_once(" -> ") {
                Some((_, new)) => new.to_string(),
                None => p.to_string(),
            }
        })
        .collect()
}

/// (code moved, docs moved) over a change set, for a docs root given
/// relative to the repository (`docs`, `documentation/`).
pub fn classify(paths: &[String], root_rel: &str) -> (Vec<String>, usize) {
    let root = root_rel.trim_end_matches('/');
    let mut code = Vec::new();
    let mut docs = 0;
    for p in paths {
        if p == root || p.starts_with(&format!("{root}/")) {
            docs += 1;
        } else if !p.ends_with(".md") {
            code.push(p.clone());
        }
    }
    (code, docs)
}

/// The docs root as git names it: relative to the repository, `/`-separated.
fn root_rel(repo: &Path, root: &Path) -> String {
    let repo_c = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let root_c = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    root_c
        .strip_prefix(&repo_c)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| root.to_string_lossy().replace('\\', "/"))
}

// ───────────────────────────── the four events

/// What a hook hands back: the exit code and what goes to stderr / stdout.
pub struct Reply {
    pub code: u8,
    pub stderr: String,
    pub stdout: String,
}

impl Reply {
    fn ok() -> Self {
        Reply {
            code: 0,
            stderr: String::new(),
            stdout: String::new(),
        }
    }
    fn block(stderr: String) -> Self {
        Reply {
            code: 2,
            stderr,
            stdout: String::new(),
        }
    }
}

const ASK: &str = "code moves with no documentation change. If a contract moved, update the page (or add the journal line) and commit; if nothing user-visible moved, run the same commit again — this gate asks once.\nThis whole Bash call was blocked — a `git add` in it did not run either. Re-run the SAME command from the start, `add` included.\n";
const DROPPED_ADD: &str = "docsys gate: the blocked call ran `git add`; this retry does not, and the working tree still has unstaged changes — did your `git add` run? Re-run the original command from the start, or stage explicitly. (asked once)\n";

/// PreToolUse on `Bash`: the commit-time question (D-040), asked once per
/// (HEAD, change set) with the marker kept until HEAD moves (D-043), and the
/// dropped-`git add` retry stopped once (D-049).
/// Does this docs root declare the knowledge-base profile? It changes what
/// the hooks guard (the record layer) and what they say (organs, not work
/// types).
/// `.docmeta.yml` `commit_policy:` — `ask` (the default: the gate asks once,
/// D-040) or `require` (D-093: nothing lands without its documentation; the
/// Stop hook holds the session until the work is recorded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitPolicy {
    Ask,
    Require,
}

pub fn commit_policy(root: &Path) -> CommitPolicy {
    let require = fs::read_to_string(root.join(".docmeta.yml")).is_ok_and(|t| {
        t.lines().any(|l| {
            l.strip_prefix("commit_policy:")
                .is_some_and(|v| v.trim().trim_matches('"') == "require")
        })
    });
    if require {
        CommitPolicy::Require
    } else {
        CommitPolicy::Ask
    }
}

/// D-093: a commit that bypassed the `require` gate (`DOCSYS_SKIP=1`) is not
/// silent — it becomes a dated debt item the next session sees. Mechanical:
/// the item is derived (date, files), not authored (R-156).
pub fn record_undocumented_commit(
    root: &Path,
    files: &[String],
    today: &str,
) -> Result<(), String> {
    let path = root.join("work/debt.md");
    let mut text = fs::read_to_string(&path).unwrap_or_else(|_| "# Debt\n".to_string());
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let shown: Vec<&str> = files.iter().take(5).map(String::as_str).collect();
    let more = files.len().saturating_sub(shown.len());
    let tail = if more > 0 {
        format!(" (+{more} more)")
    } else {
        String::new()
    };
    text.push_str(&format!(
        "- [ ] {today} committed without documentation (DOCSYS_SKIP): {}{tail} -- deferred: the session bypassed the gate -- repay when: the next session in this tree names the work and records it\n",
        shown.join(", ")
    ));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, text).map_err(|e| e.to_string())
}

const REQUIRE: &str = "commit_policy: require — nothing lands without its documentation. Name the work (feature | bug | improvement | research), record it — a work file under work/<category>/ or, at minimum, a journal entry linking these files and saying why — stage it, and run the SAME commit again, `git add` included. DOCSYS_SKIP=1 bypasses once and leaves a debt item in work/debt.md.\nThis whole Bash call was blocked — a `git add` in it did not run either.\n";

pub fn is_knowledge_base(root: &Path) -> bool {
    fs::read_to_string(root.join(".docmeta.yml")).is_ok_and(|t| {
        t.lines().any(|l| {
            l.strip_prefix("profile:")
                .is_some_and(|v| v.trim() == "knowledge-base")
        })
    })
}

/// The `raw/`-relative name of an EXISTING record the tool is about to write
/// over; `None` for anything else — a new file under `raw/inbox/` is a
/// capture and passes.
fn raw_record(repo: &Path, root: &Path, file: &str) -> Option<String> {
    let path = if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        repo.join(file)
    };
    if !path.is_file() {
        return None;
    }
    let raw = root.join("raw").canonicalize().ok()?;
    let p = path.canonicalize().ok()?;
    let rel = p.strip_prefix(&raw).ok()?;
    Some(format!("raw/{}", rel.to_string_lossy().replace('\\', "/")))
}

pub fn pre_tool_use(repo: &Path, root: &Path, payload: &str, skip: bool) -> Reply {
    let json = parse_json(payload);
    // A knowledge base's record layer: an existing file under raw/ is never
    // overwritten or edited through the agent's tools (R-023) — the one write
    // that is irreversible, and the one this hook blocks (D-076).
    if let Some(j) = &json {
        let tool = j.string_at(&["tool_name"]).unwrap_or("");
        if matches!(tool, "Write" | "Edit" | "MultiEdit" | "NotebookEdit")
            && is_knowledge_base(root)
        {
            if let Some(rel) = j
                .string_at(&["tool_input", "file_path"])
                .and_then(|f| raw_record(repo, root, f))
            {
                return Reply::block(format!(
                    "docsys: raw/ is the record and content-immutable (R-023): `{rel}` already \
                     exists and is never edited or overwritten. New knowledge is a NEW file under \
                     raw/inbox/; a processed note moves with `docsys raw move <record> <domain>` — \
                     bytes untouched, every citing page's sources: rewritten (R-027).\n"
                ));
            }
        }
    }
    let Some(cmd) = json.and_then(|j| j.string_at(&["tool_input", "command"]).map(str::to_string))
    else {
        return Reply::ok();
    };
    if !is_git_commit(&cmd) {
        return Reply::ok();
    }
    let policy = commit_policy(root);
    if skip {
        // D-093: under `require`, the bypass is recorded as debt, never silent
        if policy == CommitPolicy::Require {
            if let Ok((g, _)) = gate::run(repo, root) {
                if !g.code.is_empty() && g.docs == 0 {
                    let _ = record_undocumented_commit(root, &g.code, &crate::migrate::today());
                }
            }
        }
        return Reply::ok();
    }
    let (g, report) = match gate::run(repo, root) {
        Ok(x) => x,
        Err(e) => return Reply::block(format!("docsys gate: {e}\n")),
    };
    let mut err = String::new();
    if g.lint_errors > 0 {
        for f in &report.findings {
            err.push_str(&format!(
                "{} {} {} [{}] {}\n",
                f.severity.tag(),
                f.rule,
                f.file,
                f.subject,
                f.message
            ));
        }
        err.push_str("docsys gate: lint errors block this commit — fix them first (DOCSYS_SKIP=1 to bypass once). This whole Bash call was blocked, any `git add` in it included.\n");
        return Reply::block(err);
    }
    if !g.plan_files.is_empty() {
        return Reply::block(format!(
            "docsys gate: a seed plan is in this commit ({}) — a plan is a conversation's draft, \
             never documentation (R-003, D-091). `git reset -- {}`, land its rows with \
             `docsys seed apply --plan {} --repo . --root <docs>`, and commit the docs root only. \
             This whole Bash call was blocked, any `git add` in it included.\n",
            g.plan_files.join(", "),
            g.plan_files.join(" "),
            g.plan_files
                .first()
                .map(String::as_str)
                .unwrap_or("SEED.tsv")
        ));
    }
    if g.code.is_empty() || g.docs > 0 {
        return Reply::ok();
    }
    if policy == CommitPolicy::Require {
        // D-093: not a question — a refusal, every time, until the work is recorded
        let head_lines: Vec<&str> = g.code.iter().take(5).map(String::as_str).collect();
        let more = g.code.len().saturating_sub(head_lines.len());
        let tail = if more > 0 {
            format!(" (+{more} more)")
        } else {
            String::new()
        };
        return Reply::block(format!(
            "GATE {} changes with no docs change: {}{tail}\n{REQUIRE}",
            g.scope,
            head_lines.join(", ")
        ));
    }
    // the question — asked once per (HEAD, change set)
    let head =
        git_first(repo, &["rev-parse", "-q", "--verify", "HEAD"]).unwrap_or_else(|| "none".into());
    let mut set = git_lines(repo, &["diff", "--cached", "--name-only"]);
    if set.is_empty() {
        set = git_lines(repo, &["diff", "--name-only"]);
    }
    let key = cksum(&format!("{head}\n{}\n", set.join("\n")));
    let dir = marker_dir(repo);
    let _ = fs::create_dir_all(&dir);
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with(&format!("{head}.")) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    let marker = dir.join(format!("{head}.{key}"));
    let adds = has_git_add(&cmd);
    if !marker.exists() {
        let _ = fs::write(&marker, format!("add={}\n", u8::from(adds)));
        let head_lines: Vec<&str> = g.code.iter().take(5).map(String::as_str).collect();
        let more = g.code.len().saturating_sub(head_lines.len());
        let tail = if more > 0 {
            format!(" (+{more} more)")
        } else {
            String::new()
        };
        return Reply::block(format!(
            "GATE {} changes with no docs change: {}{tail}\n{ASK}",
            g.scope,
            head_lines.join(", ")
        ));
    }
    let asked_with_add = fs::read_to_string(&marker)
        .map(|s| s.starts_with("add=1"))
        .unwrap_or(false);
    let retry = dir.join(format!("{head}.{key}.retry"));
    if !adds
        && asked_with_add
        && !git_lines(repo, &["diff", "--name-only"]).is_empty()
        && !retry.exists()
    {
        let _ = fs::write(&retry, "");
        return Reply::block(DROPPED_ADD.to_string());
    }
    Reply::ok()
}

fn marker_dir(repo: &Path) -> PathBuf {
    let git_dir = git_first(repo, &["rev-parse", "--git-dir"])
        .map(PathBuf::from)
        .map(|p| if p.is_absolute() { p } else { repo.join(p) })
        .unwrap_or_else(std::env::temp_dir);
    git_dir.join("docsys-gate")
}

/// A small stable hash for marker names (not cryptographic; identity only).
fn cksum(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Stop: end-of-turn reminder over the working tree AND the commits not yet
/// pushed (D-041). Warns on stderr, never blocks (R-150).
pub fn stop(repo: &Path, root: &Path, payload: &str) -> Reply {
    if is_knowledge_base(root) {
        return stop_kb(repo, root);
    }
    // D-093: under `require` the end of a turn is where the session's knowledge
    // is captured — the commit may come later, from another session or a
    // person at a terminal, when this conversation is gone. The hook holds
    // the session once (Claude Code sets `stop_hook_active` on the retry).
    let require = commit_policy(root) == CommitPolicy::Require;
    let already_held = parse_json(payload)
        .and_then(|j| match j {
            Json::Obj(fields) => fields
                .iter()
                .find(|(k, _)| k == "stop_hook_active")
                .map(|(_, v)| *v == Json::Bool(true)),
            _ => None,
        })
        .unwrap_or(false);
    let hold = |text: String| -> Reply {
        if require && !already_held {
            Reply::block(format!(
                "{text}commit_policy: require — before this session ends, name the work (feature | bug | improvement | research) and record it: a work file under work/<category>/ or a journal entry linking the files and saying why. Then stop.\n"
            ))
        } else {
            Reply {
                code: 0,
                stderr: text,
                stdout: String::new(),
            }
        }
    };
    let mut paths = porcelain_paths(&git_lines(repo, &["status", "--porcelain"]));
    let ahead = git_lines(repo, &["diff", "--name-only", "@{u}..HEAD"]);
    paths.extend(ahead.iter().cloned());
    paths.sort();
    paths.dedup();
    let root_rel = root_rel(repo, root);
    let (code, docs) = classify(&paths, &root_rel);
    if code.is_empty() {
        return Reply::ok();
    }
    let where_ = if ahead.is_empty() {
        "this session"
    } else {
        "this session (including commits not yet pushed)"
    };
    if docs > 0 {
        // Documentation moved with the code — but the session's own record is
        // the journal line (R-101), and a touched draft or a renamed token in
        // a research note is not one. Found live: a public rename updated the
        // research page that named the method and ended with no journal entry.
        let root = root_rel.trim_end_matches('/');
        let journal = if root.is_empty() || root == "." {
            "work/journal.md".to_string()
        } else {
            format!("{root}/work/journal.md")
        };
        if paths.iter().any(|p| p == &journal) {
            return Reply::ok();
        }
        return hold(format!(
            "docs: {where_} changed code and documentation, but work/journal.md did\nnot move — end of session: one journal entry (≤5 lines, links not content).\n"
        ));
    }
    hold(format!(
        "docs: {where_} changed code but no documentation — if a contract\nmoved, the page moves in the SAME session; at minimum add the journal line.\n"
    ))
}

/// Is this edited file a live docs page (under the root, `.md`, not a record
/// or a template)? Paths arrive absolute or repo-relative.
/// A knowledge base has no code-without-docs question; its end-of-turn
/// nudge names what waits: notes in the inbox, errors the gate will stop.
/// Warns, never blocks (R-150).
fn stop_kb(repo: &Path, root: &Path) -> Reply {
    let notes = fs::read_dir(root.join("raw/inbox"))
        .map(|it| {
            it.filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
                .count()
        })
        .unwrap_or(0);
    let (report, _) = crate::lint_in(root, Some(repo));
    let errors = report
        .findings
        .iter()
        .filter(|f| f.severity == crate::model::Severity::Error)
        .count();
    let mut msg = String::new();
    if notes > 0 {
        msg.push_str(&format!(
            "base: {notes} note(s) waiting in raw/inbox — `process my inbox` distils them when you are ready.\n"
        ));
    }
    if errors > 0 {
        msg.push_str(&format!(
            "base: docsys lint reports {errors} error(s) — the gate stops the next commit until they are fixed.\n"
        ));
    }
    Reply {
        code: 0,
        stderr: msg,
        stdout: String::new(),
    }
}

/// The page the tool just wrote, if it is a live docs page: under the root,
/// `.md`, not a template, an archive, a materialization, the agent layer —
/// and never a `raw/` record, which no tooling edits (R-023). Canonical
/// paths, so a base that is its own repository (`--root .`) works too.
fn is_live_in(repo: &Path, root: &Path, file: &str) -> bool {
    let path = if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        repo.join(file)
    };
    let (Ok(p), Ok(r)) = (path.canonicalize(), root.canonicalize()) else {
        return false;
    };
    let Ok(tail) = p.strip_prefix(&r) else {
        return false;
    };
    let tail = tail.to_string_lossy().replace('\\', "/");
    tail.ends_with(".md")
        && !tail.starts_with("_archive/")
        && !tail.starts_with("_templates/")
        && !tail.starts_with(".federation/")
        && !tail.starts_with(".claude/")
        && !tail.starts_with("raw/")
}

pub fn is_live_page(file: &str, root_rel: &str) -> bool {
    let root = root_rel.trim_end_matches('/');
    let f = file.replace('\\', "/");
    let inside = f.starts_with(&format!("{root}/")) || f.contains(&format!("/{root}/"));
    let tail = f
        .rsplit_once(&format!("{root}/"))
        .map(|(_, t)| t)
        .unwrap_or("");
    inside
        && f.ends_with(".md")
        && !tail.starts_with("_archive/")
        && !tail.starts_with("_templates/")
}

/// PostToolUse on `Edit|Write`: keep `updated:` honest on the edited page
/// (R-052). `today` is injected so the rewrite is testable.
pub fn post_tool_use(repo: &Path, root: &Path, payload: &str, today: &str) -> Reply {
    let Some(file) = parse_json(payload).and_then(|j| {
        j.string_at(&["tool_input", "file_path"])
            .map(str::to_string)
    }) else {
        return Reply::ok();
    };
    if !is_live_in(repo, root, &file) {
        return Reply::ok();
    }
    let path = if Path::new(&file).is_absolute() {
        PathBuf::from(&file)
    } else {
        repo.join(&file)
    };
    if let Ok(text) = fs::read_to_string(&path) {
        if let Some(bumped) = bump_updated(&text, today) {
            let _ = fs::write(&path, bumped);
        }
    }
    Reply::ok()
}

/// The page with its `updated:` line set to `today`; `None` when the page
/// has no such line (nothing to keep honest) or already says so.
pub fn bump_updated(text: &str, today: &str) -> Option<String> {
    let mut changed = false;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if line.starts_with("updated:") {
            let nl = if line.ends_with('\n') { "\n" } else { "" };
            let new = format!("updated: {today}{nl}");
            if new != line {
                changed = true;
            }
            out.push_str(&new);
        } else {
            out.push_str(line);
        }
    }
    changed.then_some(out)
}

/// The knowledge base's first-turn text: organs, not work types (D-076).
pub const KB_ROUTING: &str = "<session-doc-routing>
First turn, knowledge base. Name the organ before anything else — capture,
ingest, audit or lookup; the skill of that name carries the discipline.

capture → ONE new file in raw/inbox/, the note in the user's own words plus
one line on why it is worth keeping; never classify, never touch wiki/.
ingest → one wiki page per note (id, type, domain, verification: unverified,
sources), routed from the domain index; the note moves to raw/<domain>/ with
`docsys raw move <record> <domain>` (bytes untouched, citing pages' sources
rewritten by the tool). audit → only in a session that did not write the
page; `verified` records verified_by and verified_rev. lookup → `docsys
lookup <words>` first, then the page; \"not in the base\" is a complete answer.

raw/ is content-immutable: an existing record is never edited or deleted, and
the hook blocks the attempt. A wiki page whose body changes is unverified
again. Gate: docsys lint (inside the repository).
Speak the person's language, turn by turn — the one they just wrote in; every
file under wiki/ (pages, indexes, open-questions.md) keeps the base's declared
language, whatever the session's own language setting says; code identifiers
are never translated.
Character and boundaries: AGENTS.md → ## Character.
</session-doc-routing>
";

/// The first session of a base whose character is unset (D-083): propose,
/// ask, write, summarize — in the person's language.
pub const FIRST_RUN: &str = "<first-run>
This base has no character yet (AGENTS.md → ## Character is unset). Before
anything else, propose one and ask the person to confirm or change it: a short
survey, one message, defaults offered, in the language the person just wrote
in. Five items:
1. Name — what they call the assistant (offer one).
2. Address — how the assistant addresses them: name, formal or informal.
3. Tone — plain and brief, warm, or formal; humor or none.
4. Languages — conversation mirrors the person, turn by turn; pages keep the
   base's default_content_language; code identifiers are never translated.
5. Never — what it must never do (defaults: invent what the base does not
   hold, act outward without confirmation, edit a record, verify its own page).
When the person answers, replace the placeholder block under ## Character in
AGENTS.md with the answers (keep everything else in the file), then give a
five-line summary of how you will communicate from now on, and continue with
what the person actually asked. Until they answer, use the defaults.
</first-run>
";

pub const ROUTING: &str = "<session-doc-routing>
First turn. Name the work type before anything else — one of feature, bug,
improvement (refactor, performance, cleanup), research, idea-note. If the
message makes it clear, state it in one line and proceed; when it is genuinely
ambiguous, ask THAT question first. Read <docs-in-hand> below: this tree already
holds documentation, and the work you are about to do may already have a page
or a work file.

Routing: feature needing a design decision → work/features/ (status: draft);
bug → root cause first: wrong line = journal line, wrong assumption = invariant
in reference/ or a postmortem (test: can it recur?); improvement touching a
public surface → reference/ updated, and always record WHY; research = a question
with no decision yet → work/research/ (Question · Tried · Learned · Why no
decision), no code; idea → journal or roadmap line, never the permanent layer
before it becomes a decision.

Contract-surface changes update their documentation in the SAME session.
A permanent page you write from evidence, or change in substance, carries
`verification: unverified` (+ sources); a maintainer verifies it in another
session — `verified_by:` and `confirmed:` name someone in .docmeta.yml
`maintainers:` (R-208). Nothing you write is the truth yet; say so in the page.
Inside docs a page is linked as [[dir/id]] (full path); `doc: <id>` is how
code cites a page. An id is unique across the whole tree, drafts included.
End of session: journal line (≤5 lines, links not content). Gate: docsys lint.
Judgment calls follow the procedures: docsys rules --procedures.
</session-doc-routing>
";

/// UserPromptSubmit: the routing text, once per session (stdout joins the
/// context on this event only).
pub fn user_prompt_submit(payload: &str, root: &Path) -> Reply {
    let session = parse_json(payload)
        .and_then(|j| j.string_at(&["session_id"]).map(str::to_string))
        .unwrap_or_else(|| "unknown".into());
    let safe: String = session
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let marker = std::env::temp_dir().join(format!(".docsys-intent-{safe}"));
    if marker.exists() {
        return Reply::ok();
    }
    let _ = fs::write(&marker, "");
    let text = if is_knowledge_base(root) {
        // a base without a character runs the survey first (D-083)
        let unset = fs::read_to_string(root.join("AGENTS.md"))
            .is_ok_and(|t| t.contains(crate::agents::CHARACTER_UNSET));
        if unset {
            format!("{FIRST_RUN}{KB_ROUTING}")
        } else {
            KB_ROUTING.to_string()
        }
    } else {
        format!("{ROUTING}{}", tree_digest(root))
    };
    Reply {
        code: 0,
        stderr: String::new(),
        stdout: text,
    }
}

/// What this tree already holds, for the first turn (D-093): the work in
/// flight, the open items, the commit policy — so the agent starts from the
/// documentation it has, not from a blank page.
fn tree_digest(root: &Path) -> String {
    let Ok(tree) = crate::tree::DocTree::load(root) else {
        return String::new();
    };
    use crate::fm::Value;
    use crate::tree::Kind;
    let mut flight: Vec<String> = tree
        .pages
        .iter()
        .filter(|p| p.kind == Kind::Tracked)
        .filter_map(|p| {
            let st =
                p.fm.as_ref()?
                    .fields
                    .get("status")
                    .and_then(Value::as_str)?;
            matches!(st, "draft" | "active" | "done").then(|| format!("{} ({st})", p.rel))
        })
        .collect();
    flight.sort();
    let shown: Vec<String> = flight.iter().take(8).cloned().collect();
    let more = flight.len().saturating_sub(shown.len());
    let permanent = tree
        .pages
        .iter()
        .filter(|p| p.kind == Kind::Permanent)
        .count();
    let unverified = tree
        .pages
        .iter()
        .filter(|p| p.kind == Kind::Permanent)
        .filter(|p| {
            p.fm.as_ref()
                .and_then(|f| f.fields.get("verification"))
                .and_then(Value::as_str)
                == Some("unverified")
        })
        .count();
    let mut out = String::from("<docs-in-hand>\n");
    out.push_str(&format!(
        "{permanent} permanent page(s){}; index.md routes them; `docsys lookup <words>` before writing a page.\n",
        if unverified > 0 {
            format!(", {unverified} unverified")
        } else {
            String::new()
        }
    ));
    if shown.is_empty() {
        out.push_str("Work in flight: none.\n");
    } else {
        out.push_str(&format!(
            "Work in flight: {}{}\n",
            shown.join(", "),
            if more > 0 {
                format!(" (+{more} more)")
            } else {
                String::new()
            }
        ));
    }
    if commit_policy(root) == CommitPolicy::Require {
        out.push_str("commit_policy: require — no commit lands without its documentation, and the end of a turn holds until the work is recorded (feature | bug | improvement | research → work file or journal entry).\n");
    }
    out.push_str("</docs-in-hand>\n");
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn json_strings_decode_every_escape() {
        let j = parse_json(
            r#"{"a":"q\"b\\c\/d\n\t\u00e7\ud83d\ude00","n":[1,-2.5e3,true,null],"o":{"k":"v"}}"#,
        )
        .unwrap();
        assert_eq!(j.string_at(&["a"]), Some("q\"b\\c/d\n\tç😀"));
        assert_eq!(j.string_at(&["o", "k"]), Some("v"));
        assert_eq!(j.string_at(&["n"]), None);
        assert_eq!(j.string_at(&["missing"]), None);
        assert_eq!(parse_json(" { } "), Some(Json::Obj(vec![])));
    }

    #[test]
    fn malformed_json_is_none_never_a_guess() {
        for bad in [
            "",
            "not json",
            "{\"a\":}",
            "{\"a\":\"open}",
            "{\"a\":1} x",
            "{\"a\":\"\\ud83d\"}",
            "{\"a\":\"\\q\"}",
        ] {
            assert_eq!(parse_json(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn heredoc_bodies_are_dropped_and_the_heredoc_line_kept() {
        let cmd = "cat > r.md <<'EOF'\nrun git commit later\nEOF\necho done";
        assert_eq!(command_lines(cmd), vec!["cat > r.md <<'EOF'", "echo done"]);
        assert!(!is_git_commit(cmd));
        assert!(is_git_commit(
            "git commit -q -F - <<'MSG'\nbody\nMSG\ngit push"
        ));
        assert!(is_git_commit(
            "cat <<EOF\ngit commit\nEOF\ngit commit -m real"
        ));
        // <<- strips leading tabs on the terminator
        assert!(!is_git_commit("cat <<-\tEOF\n\tgit commit\n\tEOF"));
        // an unterminated heredoc swallows the rest — nothing after it runs
        assert!(!is_git_commit("cat <<EOF\ngit commit\n"));
        // a number is not a terminator word; `<<` as an operator is not a heredoc
        assert!(is_git_commit("x=$((1<<3)); git commit -m y"));
    }

    #[test]
    fn commit_detection_table() {
        let cases: &[(&str, bool)] = &[
            ("git commit -m x", true),
            ("git   commit -m x", false),
            ("git add -A && git commit -m x", true),
            ("printf \"x\" > y && git commit -m z", true),
            ("git status && git log --oneline -3", false),
            ("grep -rn 'git commit' docs/", true),
            ("gitk && git-commit-graph", false),
            ("echo çekirdek && git commit -m \"günlük\"", true),
            ("", false),
        ];
        for (cmd, want) in cases {
            assert_eq!(is_git_commit(cmd), *want, "{cmd:?}");
        }
        assert!(has_git_add("git add -u src && git commit"));
        assert!(!has_git_add("git commit -m x"));
    }

    #[test]
    fn payload_without_a_command_is_not_a_commit() {
        let t = std::env::temp_dir();
        for p in [
            "",
            "not json",
            r#"{"tool_input":{}}"#,
            r#"{"other":"git commit"}"#,
            r#"{"tool_input":{"command":7}}"#,
        ] {
            let r = pre_tool_use(&t, &t.join("docs"), p, false);
            assert_eq!(r.code, 0, "{p:?}");
        }
    }

    #[test]
    fn porcelain_paths_take_the_new_side_of_a_rename() {
        let lines: Vec<String> = [
            "?? new.rs",
            " M docs/a.md",
            "R  old.md -> docs/b.md",
            "A  kaynak/çekirdek.rs",
            "D  gone.rs",
            "",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            porcelain_paths(&lines),
            vec![
                "new.rs",
                "docs/a.md",
                "docs/b.md",
                "kaynak/çekirdek.rs",
                "gone.rs"
            ]
        );
    }

    #[test]
    fn classification_by_root_and_extension() {
        let paths: Vec<String> = [
            "docs/günlük.md",
            "src/a.rs",
            "AGENTS.md",
            "docs",
            "documentation/x.md",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let (code, docs) = classify(&paths, "docs/");
        assert_eq!(code, vec!["src/a.rs"]);
        assert_eq!(docs, 2);
        // under another root, `docs` is just a path without `.md`: code
        let (code, docs) = classify(&paths, "documentation");
        assert_eq!(code, vec!["src/a.rs", "docs"]);
        assert_eq!(docs, 1);
    }

    #[test]
    fn live_page_filter() {
        assert!(is_live_page("/r/docs/reference/a.md", "docs"));
        assert!(is_live_page("docs/reference/kılavuz.md", "docs/"));
        assert!(!is_live_page("/r/docs/_archive/a.md", "docs"));
        assert!(!is_live_page("/r/docs/_templates/a.md", "docs"));
        assert!(!is_live_page("/r/src/a.rs", "docs"));
        assert!(!is_live_page("/r/docs/a.txt", "docs"));
        assert!(!is_live_page("/r/mydocs/a.md", "docs"));
    }

    #[test]
    fn updated_is_bumped_only_when_present_and_stale() {
        assert_eq!(
            bump_updated("---\nid: a\nupdated: 2026-01-01\n---\nbody\n", "2026-08-26").as_deref(),
            Some("---\nid: a\nupdated: 2026-08-26\n---\nbody\n")
        );
        assert_eq!(bump_updated("---\nid: a\n---\n", "2026-08-26"), None);
        assert_eq!(bump_updated("updated: 2026-08-26\n", "2026-08-26"), None);
        assert_eq!(
            bump_updated("updated: x", "2026-08-26").as_deref(),
            Some("updated: 2026-08-26")
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod render_tests {
    use super::*;

    #[test]
    fn what_render_writes_the_reader_reads_back_unchanged() {
        let text = r#"{"a":[1,true,null,"q\"\\\n\u0001é"],"b":{},"c":[],"d":{"e":-2.5e3}}"#;
        let v = parse_json(text).unwrap();
        let out = v.render();
        assert_eq!(parse_json(&out).unwrap(), v, "{out}");
        assert!(out.starts_with("{\n  \"a\": [\n    1,\n"), "{out}");
        assert!(out.contains("  \"b\": {},\n  \"c\": [],\n"), "{out}");
        assert!(out.ends_with("}\n"), "{out}");
    }
}
