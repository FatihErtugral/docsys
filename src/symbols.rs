//! Symbol resolution for `verifies:` pins (R-114, D-106): a symbol names a
//! declaration, read by a small grammar per extension family over a token
//! stream that skips comments and string literals. A use is never a
//! declaration; a symbol that is absent, only used, or declared more than
//! once is refused with the lines that make it so — never guessed.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Family {
    Rust,
    Script,
    Python,
    Go,
    Brace,
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn extension(path: &str) -> String {
    file_name(path)
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}

fn family(path: &str) -> Family {
    match extension(path).as_str() {
        "rs" => Family::Rust,
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" | "vue" | "svelte" => {
            Family::Script
        }
        "py" | "pyi" => Family::Python,
        "go" => Family::Go,
        _ => Family::Brace,
    }
}

/// A single-file component with everything outside its `<script>` blocks
/// blanked and every newline kept, so line numbers stay the file's.
fn script_blocks(source: &str) -> String {
    let mut out: Vec<u8> = source
        .bytes()
        .map(|b| if b == b'\n' { b'\n' } else { b' ' })
        .collect();
    let mut from = 0;
    while let Some(open) = source
        .get(from..)
        .and_then(|s| s.find("<script"))
        .map(|p| p + from)
    {
        let Some(gt) = source
            .get(open..)
            .and_then(|s| s.find('>'))
            .map(|p| open + p)
        else {
            break;
        };
        let body = gt + 1;
        if source.get(gt.saturating_sub(1)..gt) == Some("/") {
            from = body;
            continue;
        }
        let end = source
            .get(body..)
            .and_then(|s| s.find("</script"))
            .map_or(source.len(), |p| body + p);
        for (slot, b) in out
            .iter_mut()
            .zip(source.bytes())
            .skip(body)
            .take(end.saturating_sub(body))
        {
            *slot = b;
        }
        from = end;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------- tokens

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Word,
    Punct,
    Str,
}

#[derive(Clone, Copy, Debug)]
struct Tok<'a> {
    kind: Kind,
    text: &'a str,
    line: usize,
    /// the line the token ends on: a string or template can span lines
    end: usize,
}

/// Longest first.
const OPS: [&str; 24] = [
    "===", "!==", "...", "::", "->", "=>", "<-", "==", "!=", "<=", ">=", "&&", "||", "??", "?.",
    "+=", "-=", "*=", "/=", "%=", "|=", "&=", "^=", ":=",
];

fn at(b: &[u8], i: usize) -> u8 {
    b.get(i).copied().unwrap_or(0)
}

fn is_word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

struct Lexer<'a> {
    src: &'a str,
    b: &'a [u8],
    fam: Family,
    i: usize,
    line: usize,
    toks: Vec<Tok<'a>>,
    /// the brace depth at which each open `${` of a template began
    interp: Vec<usize>,
    braces: usize,
}

fn lex(src: &str, fam: Family) -> Vec<Tok<'_>> {
    let mut lx = Lexer {
        src,
        b: src.as_bytes(),
        fam,
        i: 0,
        line: 0,
        toks: Vec::new(),
        interp: Vec::new(),
        braces: 0,
    };
    while lx.i < lx.b.len() {
        lx.step();
    }
    lx.toks
}

impl<'a> Lexer<'a> {
    fn peek(&self, k: usize) -> u8 {
        at(self.b, self.i + k)
    }

    fn push(&mut self, kind: Kind, from: usize, line: usize) {
        self.toks.push(Tok {
            kind,
            text: self.src.get(from..self.i).unwrap_or(""),
            line,
            end: self.line,
        });
    }

    fn step(&mut self) {
        let c = self.peek(0);
        let py = self.fam == Family::Python;
        match c {
            b'\n' => {
                self.line += 1;
                self.i += 1;
            }
            _ if c.is_ascii_whitespace() => self.i += 1,
            b'/' if !py && self.peek(1) == b'/' => self.skip_line(),
            b'#' if py => self.skip_line(),
            b'/' if !py && self.peek(1) == b'*' => self.block_comment(),
            b'"' | b'\'' | b'`' => self.quote(c, self.i),
            _ if is_word_byte(c) => self.word(),
            _ => self.punct(),
        }
    }

    fn skip_line(&mut self) {
        while self.i < self.b.len() && self.peek(0) != b'\n' {
            self.i += 1;
        }
    }

    fn block_comment(&mut self) {
        self.i += 2;
        let mut depth = 1;
        while self.i < self.b.len() {
            match (self.peek(0), self.peek(1)) {
                (b'\n', _) => self.line += 1,
                (b'/', b'*') if self.fam == Family::Rust => {
                    depth += 1;
                    self.i += 1;
                }
                (b'*', b'/') => {
                    depth -= 1;
                    self.i += 2;
                    if depth == 0 {
                        return;
                    }
                    continue;
                }
                _ => {}
            }
            self.i += 1;
        }
    }

    /// A quote at `self.i`; `from` is where the literal began (a prefix
    /// such as Python's `rb` or Rust's `b` belongs to it).
    fn quote(&mut self, q: u8, from: usize) {
        let line = self.line;
        let prev_word = from > 0 && is_word_byte(at(self.b, from - 1));
        let triple = self.peek(1) == q && self.peek(2) == q;
        match (self.fam, q) {
            (Family::Rust, b'\'') => self.rust_tick(from),
            (Family::Brace, b'\'') if prev_word => self.punct(),
            (Family::Script, b'`') => self.template(from, line),
            (Family::Go, b'`') => self.string(from, line, 1, true, false),
            (Family::Rust | Family::Python, b'`') => self.punct(),
            (Family::Python | Family::Brace, b'"' | b'\'') if triple => {
                self.string(from, line, 3, true, true)
            }
            (Family::Rust, _) => self.string(from, line, 1, true, true),
            _ => self.string(from, line, 1, false, true),
        }
    }

    /// A string whose quote (`len` bytes) starts at `self.i`.
    fn string(&mut self, from: usize, line: usize, len: usize, multiline: bool, escapes: bool) {
        let close: Vec<u8> = self
            .b
            .get(self.i..self.i + len)
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        self.i += len;
        while self.i < self.b.len() {
            let c = self.peek(0);
            if escapes && c == b'\\' {
                if self.peek(1) == b'\n' {
                    self.line += 1;
                }
                self.i += 2;
                continue;
            }
            if c == b'\n' {
                if !multiline {
                    break;
                }
                self.line += 1;
            }
            if self.b.get(self.i..self.i + len) == Some(close.as_slice()) {
                self.i += len;
                break;
            }
            self.i += 1;
        }
        self.i = self.i.min(self.b.len());
        self.push(Kind::Str, from, line);
    }

    /// Rust's `'`: a char literal, or the tick of a lifetime.
    fn rust_tick(&mut self, from: usize) {
        let line = self.line;
        if self.peek(1) == b'\\' {
            self.string(from, line, 1, false, true);
        } else if self.peek(2) == b'\'' && self.peek(1) != b'\n' {
            self.i += 3;
            self.push(Kind::Str, from, line);
        } else if self.peek(1) >= 0x80 {
            let close = (2..6).find(|k| self.peek(*k) == b'\'');
            match close {
                Some(k) => {
                    self.i += k + 1;
                    self.push(Kind::Str, from, line);
                }
                None => self.punct(),
            }
        } else {
            self.punct();
        }
    }

    /// A template literal, from its backtick or from the `}` that closes
    /// one of its `${…}`. The code inside `${…}` is tokenized as code.
    fn template(&mut self, from: usize, line: usize) {
        self.i += 1;
        while self.i < self.b.len() {
            match self.peek(0) {
                b'\\' => {
                    if self.peek(1) == b'\n' {
                        self.line += 1;
                    }
                    self.i += 2;
                }
                b'\n' => {
                    self.line += 1;
                    self.i += 1;
                }
                b'`' => {
                    self.i += 1;
                    break;
                }
                b'$' if self.peek(1) == b'{' => {
                    self.i += 2;
                    self.push(Kind::Str, from, line);
                    self.interp.push(self.braces);
                    return;
                }
                _ => self.i += 1,
            }
        }
        self.i = self.i.min(self.b.len());
        self.push(Kind::Str, from, line);
    }

    fn word(&mut self) {
        let from = self.i;
        let line = self.line;
        while is_word_byte(self.peek(0)) {
            self.i += 1;
        }
        let w = self.src.get(from..self.i).unwrap_or("");
        let next = self.peek(0);
        match self.fam {
            Family::Python
                if matches!(next, b'"' | b'\'')
                    && w.len() <= 2
                    && w.bytes().all(|c| b"rRbBuUfF".contains(&c)) =>
            {
                self.quote(next, from)
            }
            Family::Rust if matches!(w, "r" | "br" | "cr") && matches!(next, b'"' | b'#') => {
                self.raw_string(from, line)
            }
            Family::Rust if matches!(w, "b" | "c") && next == b'"' => {
                self.string(from, line, 1, true, true)
            }
            Family::Rust if w == "b" && next == b'\'' => self.string(from, line, 1, false, true),
            _ => self.push(Kind::Word, from, line),
        }
    }

    /// Rust's `r#"…"#`; `r#ident` is a raw identifier, left a word.
    fn raw_string(&mut self, from: usize, line: usize) {
        let mut hashes = 0;
        while self.peek(hashes) == b'#' {
            hashes += 1;
        }
        if self.peek(hashes) != b'"' {
            self.push(Kind::Word, from, line);
            return;
        }
        self.i += hashes + 1;
        while self.i < self.b.len() {
            if self.peek(0) == b'\n' {
                self.line += 1;
            }
            if self.peek(0) == b'"' && (1..=hashes).all(|k| self.peek(k) == b'#') {
                self.i += hashes + 1;
                break;
            }
            self.i += 1;
        }
        self.i = self.i.min(self.b.len());
        self.push(Kind::Str, from, line);
    }

    /// Can a `/` here start a regular expression literal?
    fn regex_allowed(&self) -> bool {
        match self.toks.last() {
            None => true,
            Some(t) => match t.kind {
                Kind::Str => false,
                Kind::Word => matches!(
                    t.text,
                    "return"
                        | "typeof"
                        | "instanceof"
                        | "in"
                        | "of"
                        | "new"
                        | "delete"
                        | "void"
                        | "throw"
                        | "case"
                        | "do"
                        | "else"
                        | "yield"
                        | "await"
                ),
                // `</tag>` in JSX is not a regex
                Kind::Punct => !matches!(t.text, ")" | "]" | "}" | "<"),
            },
        }
    }

    fn regex(&mut self) {
        let from = self.i;
        let line = self.line;
        self.i += 1;
        let mut class = false;
        while self.i < self.b.len() {
            match self.peek(0) {
                b'\n' => break,
                b'\\' if self.peek(1) != b'\n' => self.i += 1,
                b'[' => class = true,
                b']' => class = false,
                b'/' if !class => {
                    self.i += 1;
                    break;
                }
                _ => {}
            }
            self.i += 1;
        }
        while is_word_byte(self.peek(0)) {
            self.i += 1;
        }
        self.i = self.i.min(self.b.len());
        self.push(Kind::Str, from, line);
    }

    fn punct(&mut self) {
        let from = self.i;
        let line = self.line;
        if self.fam == Family::Script && self.peek(0) == b'/' && self.regex_allowed() {
            self.regex();
            return;
        }
        let rest = self.b.get(self.i..).unwrap_or(&[]);
        let len = OPS
            .iter()
            .find(|op| rest.starts_with(op.as_bytes()))
            .map_or(1, |op| op.len());
        self.i += len;
        match at(self.b, from) {
            b'{' => self.braces += 1,
            b'}' if self.interp.last() == Some(&self.braces) => {
                self.interp.pop();
                self.i = from;
                self.template(from, line);
                return;
            }
            b'}' => self.braces = self.braces.saturating_sub(1),
            _ => {}
        }
        self.push(Kind::Punct, from, line);
    }
}

// ---------------------------------------------------------------- reading

/// How a declaration's region ends (D-106).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    /// to the `}` closing the first `{` met at bracket depth 0, or a `;`
    Block,
    /// to the first `;` at depth 0, else the first line that ends at depth 0
    /// without a continuation
    Stmt,
    /// a member of a class, an object, an enum or a struct: either, and a
    /// `,` at depth 0 ends it too
    Member,
    /// Python: the header and every line indented deeper
    Indent,
    /// Python: one logical line
    Logical,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Decl {
    /// the token that names it
    name: usize,
    /// its first token: a modifier, its keyword or its name
    start: usize,
    shape: Shape,
    /// Go: a method's receiver type
    owner: Option<usize>,
    /// TS/JS: a function written as a property's or a member's value — it
    /// declares its name only where nothing else in the file does
    fallback: bool,
}

impl Decl {
    fn new(name: usize, start: usize, shape: Shape) -> Self {
        Decl {
            name,
            start,
            shape,
            owner: None,
            fallback: false,
        }
    }

    fn fallback(name: usize, start: usize, shape: Shape) -> Self {
        Decl {
            fallback: true,
            ..Decl::new(name, start, shape)
        }
    }
}

/// A declaration's lines (0-based, inclusive) and the `{` of its body.
struct Region {
    first: usize,
    last: usize,
    body: Option<usize>,
}

struct Decls {
    all: Vec<Decl>,
    /// Rust: every `impl` block, named by its self type
    impls: Vec<Decl>,
}

const TS_MODIFIERS: [&str; 12] = [
    "public",
    "private",
    "protected",
    "static",
    "readonly",
    "async",
    "get",
    "set",
    "override",
    "abstract",
    "declare",
    "accessor",
];

const NOT_A_METHOD: [&str; 31] = [
    "if",
    "for",
    "while",
    "switch",
    "catch",
    "with",
    "function",
    "return",
    "typeof",
    "new",
    "do",
    "else",
    "try",
    "finally",
    "await",
    "yield",
    "super",
    "this",
    "import",
    "export",
    "delete",
    "void",
    "throw",
    "case",
    "in",
    "of",
    "instanceof",
    "class",
    "const",
    "let",
    "var",
];

/// Lines these words lead never declare (the generic brace-language rule).
const NOT_A_DECLARATION: [&str; 18] = [
    "if", "while", "for", "switch", "catch", "return", "else", "do", "new", "throw", "case",
    "await", "try", "yield", "echo", "print", "import", "package",
];

const PY_NOT_A_NAME: [&str; 20] = [
    "else", "try", "finally", "except", "lambda", "match", "case", "if", "elif", "while", "for",
    "with", "return", "pass", "async", "await", "global", "nonlocal", "del", "assert",
];

fn indent(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

fn before(i: usize, start: usize) -> usize {
    if i > start {
        i - 1
    } else {
        start
    }
}

struct Src<'a> {
    fam: Family,
    toks: Vec<Tok<'a>>,
    pair: Vec<Option<usize>>,
    parent: Vec<Option<usize>>,
    lines: Vec<&'a str>,
    /// Python: lines that continue a logical line begun above
    cont: Vec<bool>,
}

impl<'a> Src<'a> {
    fn new(text: &'a str, source: &'a str, fam: Family) -> Self {
        let toks = lex(text, fam);
        let mut pair = vec![None; toks.len()];
        let mut parent = vec![None; toks.len()];
        let mut stack: Vec<usize> = Vec::new();
        for (i, t) in toks.iter().enumerate() {
            let mut up = stack.last().copied();
            if t.kind == Kind::Punct {
                match t.text {
                    "(" | "[" | "{" => stack.push(i),
                    ")" | "]" | "}" => {
                        if let Some(o) = stack.pop() {
                            if let Some(s) = pair.get_mut(o) {
                                *s = Some(i);
                            }
                            if let Some(s) = pair.get_mut(i) {
                                *s = Some(o);
                            }
                            up = parent.get(o).copied().flatten();
                        }
                    }
                    _ => {}
                }
            }
            if let Some(s) = parent.get_mut(i) {
                *s = up;
            }
        }
        let mut src = Src {
            fam,
            toks,
            pair,
            parent,
            lines: source.lines().collect(),
            cont: Vec::new(),
        };
        if fam == Family::Python {
            src.cont = src.continuation_lines();
        }
        src
    }

    fn n(&self) -> usize {
        self.toks.len()
    }

    /// The token's text; empty for a string literal and past the end, so a
    /// literal never reads as a keyword, a bracket or a name.
    fn text(&self, i: usize) -> &'a str {
        match self.toks.get(i) {
            Some(t) if t.kind != Kind::Str => t.text,
            _ => "",
        }
    }

    fn is(&self, i: usize, s: &str) -> bool {
        self.text(i) == s
    }

    fn word(&self, i: usize) -> bool {
        self.toks.get(i).is_some_and(|t| t.kind == Kind::Word)
    }

    fn is_str(&self, i: usize) -> bool {
        self.toks.get(i).is_some_and(|t| t.kind == Kind::Str)
    }

    fn line(&self, i: usize) -> usize {
        self.toks.get(i).map_or(0, |t| t.line)
    }

    fn end(&self, i: usize) -> usize {
        self.toks.get(i).map_or(0, |t| t.end)
    }

    fn pair(&self, i: usize) -> Option<usize> {
        self.pair.get(i).copied().flatten()
    }

    fn parent(&self, i: usize) -> Option<usize> {
        self.parent.get(i).copied().flatten()
    }

    fn line_text(&self, l: usize) -> &'a str {
        self.lines.get(l).copied().unwrap_or("")
    }

    fn first_on_line(&self, i: usize) -> bool {
        i == 0 || self.end(i - 1) < self.line(i)
    }

    fn last_on_line(&self, i: usize) -> bool {
        i + 1 >= self.n() || self.line(i + 1) > self.end(i)
    }

    /// A line ending in this token goes on (D-106's continuation characters).
    fn trailing(&self, i: usize) -> bool {
        matches!(
            self.text(i),
            "=" | ","
                | "("
                | "["
                | "{"
                | "+"
                | "-"
                | "*"
                | "/"
                | "?"
                | ":"
                | "."
                | "&&"
                | "||"
                | "=>"
                | "|"
                | "&"
                | "<"
                | "??"
                | "%"
                | "->"
                | "::"
        )
    }

    /// A line starting with this token continues the one above.
    fn leading(&self, i: usize) -> bool {
        matches!(
            self.text(i),
            "." | "?."
                | "?"
                | ":"
                | "+"
                | "-"
                | "*"
                | "/"
                | "%"
                | "&&"
                | "||"
                | "??"
                | "=>"
                | "|"
                | "&"
                | "="
                | "->"
                | "::"
                | ">"
        ) || (self.fam == Family::Script
            && matches!(self.text(i), "as" | "satisfies" | "extends" | "implements"))
    }

    /// Can a statement (or, with `comma`, a member) begin at token `i`?
    fn starts(&self, i: usize, comma: bool) -> bool {
        let Some(p) = i.checked_sub(1) else {
            return true;
        };
        match self.text(p) {
            ";" | "{" | "}" => true,
            "," if comma => true,
            _ => (self.end(p) < self.line(i) && !self.trailing(p)) || self.after_decorator(i),
        }
    }

    /// `@name`, `@a.b` or `@name(…)` right before `i`.
    fn after_decorator(&self, i: usize) -> bool {
        let Some(mut q) = i.checked_sub(1) else {
            return false;
        };
        if self.is(q, ")") {
            match self.pair(q).and_then(|o| o.checked_sub(1)) {
                Some(o) => q = o,
                None => return false,
            }
        }
        while q >= 2 && self.word(q) && self.is(q - 1, ".") {
            q -= 2;
        }
        q >= 1 && self.word(q) && self.is(q - 1, "@")
    }

    /// Past the modifiers that may lead a member header.
    fn skip_modifiers(&self, i: usize) -> usize {
        let mut j = i;
        loop {
            let t = self.text(j);
            match self.fam {
                Family::Script
                    if TS_MODIFIERS.contains(&t)
                        && (self.word(j + 1) || matches!(self.text(j + 1), "#" | "*" | "[")) =>
                {
                    j += 1
                }
                Family::Script if t == "*" || t == "#" => j += 1,
                Family::Rust if t == "pub" => {
                    j = match self.is(j + 1, "(").then(|| self.pair(j + 1)).flatten() {
                        Some(close) => close + 1,
                        None => j + 1,
                    }
                }
                _ => return j,
            }
        }
    }

    /// The `>` closing the `<` at `k`, brackets skipped whole.
    fn angle_close(&self, k: usize) -> usize {
        let mut depth = 0i64;
        let mut j = k;
        while j < self.n() {
            match self.text(j) {
                "<" => depth += 1,
                ">" => {
                    depth -= 1;
                    if depth <= 0 {
                        return j;
                    }
                }
                "(" | "[" | "{" => j = self.pair(j).unwrap_or(j),
                ";" => return j,
                _ => {}
            }
            j += 1;
        }
        j
    }

    // ------------------------------------------------------------ extents

    /// The last token of the declaration that begins at `start`, and the `{`
    /// of its body. A header may run over several lines; brackets are
    /// balanced on the way.
    fn extent(&self, start: usize, shape: Shape) -> (usize, Option<usize>) {
        // Go puts `{` on the header's line: a Go header that ends its line has no body
        let line_rule = shape == Shape::Stmt
            || (shape == Shape::Member && matches!(self.fam, Family::Script | Family::Go))
            || (shape == Shape::Block && self.fam == Family::Go);
        let comma = shape == Shape::Member;
        let (mut paren, mut angle) = (0i64, 0i64);
        let mut body = None;
        let mut i = start;
        while i < self.n() {
            match self.text(i) {
                "(" | "[" => paren += 1,
                ")" | "]" => {
                    paren -= 1;
                    if paren < 0 {
                        return (before(i, start), body);
                    }
                }
                "<" if paren == 0 => angle += 1,
                ">" if paren == 0 && angle > 0 => angle -= 1,
                "{" => {
                    let close = self.pair(i).unwrap_or(self.n().saturating_sub(1));
                    // `): { a: T } {`, `X & {…}`, Go's `struct{}`: a type, not the body
                    let type_literal = i > 0
                        && matches!(
                            self.text(i - 1),
                            ":" | "|" | "&" | "=>" | "struct" | "interface"
                        );
                    if shape != Shape::Stmt && paren == 0 && angle == 0 && !type_literal {
                        return (close, Some(i));
                    }
                    // a type, a default value: skipped whole
                    body = body.or(Some(i));
                    i = close;
                }
                "}" => return (before(i, start), body),
                ";" if paren == 0 => return (i, body),
                "," if comma && paren == 0 && angle == 0 => return (i, body),
                "=" if paren == 0 && angle == 0 => {
                    return self.value_end(i + 1, start, comma, body)
                }
                _ => {}
            }
            if line_rule
                && paren == 0
                && angle == 0
                && self.last_on_line(i)
                && !self.trailing(i)
                && !self.is(i + 1, "{")
                && !self.leading(i + 1)
            {
                return (i, body);
            }
            i += 1;
        }
        (self.n().saturating_sub(1).max(start), body)
    }

    /// The end of a declaration's value: the first `;` at depth 0, else the
    /// first line that ends at depth 0 without a continuation.
    fn value_end(
        &self,
        from: usize,
        start: usize,
        comma: bool,
        mut body: Option<usize>,
    ) -> (usize, Option<usize>) {
        let mut depth = 0i64;
        let mut i = from;
        while i < self.n() {
            match self.text(i) {
                "(" | "[" => depth += 1,
                "{" => {
                    depth += 1;
                    body = body.or(Some(i));
                }
                ")" | "]" | "}" => {
                    depth -= 1;
                    if depth < 0 {
                        return (before(i, start), body);
                    }
                }
                ";" if depth == 0 => return (i, body),
                "," if comma && depth == 0 => return (i, body),
                _ => {}
            }
            if depth == 0 && self.last_on_line(i) && !self.trailing(i) && !self.leading(i + 1) {
                return (i, body);
            }
            i += 1;
        }
        (self.n().saturating_sub(1).max(start), body)
    }

    fn region(&self, d: &Decl) -> Region {
        let first = self.line(d.start);
        match d.shape {
            Shape::Indent => Region {
                first,
                last: self.indent_block(first),
                body: None,
            },
            Shape::Logical => Region {
                first,
                last: self.end(self.logical_end(d.start)).max(first),
                body: None,
            },
            _ => {
                let (end, body) = self.extent(d.start, d.shape);
                Region {
                    first,
                    last: self.end(end).max(first),
                    body,
                }
            }
        }
    }

    // ------------------------------------------------------------ Python

    fn logical_start(&self, i: usize) -> bool {
        self.first_on_line(i)
            && self.parent(i).is_none()
            && !matches!(self.text(i), ")" | "]" | "}")
            && !(i > 0 && self.is(i - 1, "\\"))
    }

    fn logical_end(&self, start: usize) -> usize {
        let mut j = start;
        while j + 1 < self.n() && !self.logical_start(j + 1) {
            j += 1;
        }
        j
    }

    fn continuation_lines(&self) -> Vec<bool> {
        let mut cont = vec![false; self.lines.len() + 1];
        for (i, t) in self.toks.iter().enumerate() {
            for slot in cont.iter_mut().take(t.end + 1).skip(t.line + 1) {
                *slot = true;
            }
            if self.first_on_line(i) && !self.logical_start(i) {
                if let Some(slot) = cont.get_mut(t.line) {
                    *slot = true;
                }
            }
        }
        cont
    }

    fn is_cont(&self, l: usize) -> bool {
        self.cont.get(l).copied().unwrap_or(false)
    }

    /// D-069's indentation rule, read as Python reads it: the header and every
    /// following line indented deeper; a line that continues a string or a
    /// bracket is kept, and a blank or comment-only line never ends the block.
    fn indent_block(&self, first: usize) -> usize {
        let base = indent(self.line_text(first));
        let mut last = first;
        for l in first + 1..self.lines.len() {
            let text = self.line_text(l);
            let quiet = text.trim().is_empty() || text.trim_start().starts_with('#');
            if self.is_cont(l) || (!text.trim().is_empty() && indent(text) > base) {
                last = l;
            } else if !quiet {
                break;
            }
        }
        last
    }

    fn python_decls(&self) -> Vec<Decl> {
        let mut out = Vec::new();
        for i in 0..self.n() {
            if !self.word(i) || !self.logical_start(i) {
                continue;
            }
            let j = if self.is(i, "async") { i + 1 } else { i };
            if matches!(self.text(j), "def" | "class") {
                if self.word(j + 1) {
                    out.push(Decl::new(j + 1, i, Shape::Indent));
                }
            } else if !PY_NOT_A_NAME.contains(&self.text(i))
                && (self.is(i + 1, "=") || (self.is(i + 1, ":") && self.assigns(i)))
            {
                out.push(Decl::new(i, i, Shape::Logical));
            }
        }
        out
    }

    fn assigns(&self, i: usize) -> bool {
        let end = self.logical_end(i);
        (i + 1..=end).any(|k| self.is(k, "=") && self.parent(k).is_none())
    }

    // ------------------------------------------------------------ Rust

    fn rust_decls(&self, impls: &mut Vec<Decl>) -> Vec<Decl> {
        let mut out = Vec::new();
        for i in 0..self.n() {
            let found = match self.text(i) {
                "fn" | "struct" | "enum" | "trait" | "mod" => Some((i + 1, Shape::Block)),
                "union" if matches!(self.text(i + 2), "{" | "<") => Some((i + 1, Shape::Block)),
                "type" => Some((i + 1, Shape::Stmt)),
                "const" if self.is(i + 2, ":") && !(i > 0 && self.is(i - 1, "*")) => {
                    Some((i + 1, Shape::Stmt))
                }
                "static" => {
                    let k = if self.is(i + 1, "mut") { i + 2 } else { i + 1 };
                    self.is(k + 1, ":").then_some((k, Shape::Stmt))
                }
                "macro_rules" if self.is(i + 1, "!") => Some((i + 2, Shape::Block)),
                // an impl block starts an item; `impl Trait` in a signature
                // is a type
                "impl" if self.item_starts(self.rust_start(i)) => {
                    if let Some(t) = self.impl_self(i) {
                        impls.push(Decl::new(t, self.rust_start(i), Shape::Block));
                    }
                    None
                }
                _ => None,
            };
            if let Some((k, shape)) = found.filter(|(k, _)| self.word(*k)) {
                out.push(Decl::new(k, self.rust_start(i), shape));
            }
        }
        out
    }

    /// Whether an item can start at token `s`: the file's start, or after
    /// a `;`, a block's brace, or an attribute's `]`.
    fn item_starts(&self, s: usize) -> bool {
        s == 0 || matches!(self.text(s - 1), ";" | "}" | "{" | "]")
    }

    /// Back over `pub(…)`, `async`, `unsafe`, `const`, `extern "…"`.
    fn rust_start(&self, i: usize) -> usize {
        let mut s = i;
        while let Some(p) = s.checked_sub(1) {
            let pub_group = self
                .is(p, ")")
                .then(|| self.pair(p))
                .flatten()
                .and_then(|o| o.checked_sub(1))
                .filter(|o| self.is(*o, "pub"));
            if matches!(
                self.text(p),
                "pub" | "async" | "unsafe" | "const" | "extern" | "default"
            ) {
                s = p;
            } else if self.is_str(p) && p > 0 && self.is(p - 1, "extern") {
                s = p - 1;
            } else if let Some(o) = pub_group {
                s = o;
            } else {
                break;
            }
        }
        s
    }

    /// The self type of the `impl` at `i`: the last name at angle depth 0
    /// after `for`, or after the generics when there is no `for`.
    fn impl_self(&self, i: usize) -> Option<usize> {
        let mut k = i + 1;
        if self.is(k, "<") {
            k = self.angle_close(k) + 1;
        }
        let mut angle = 0i64;
        let mut last = None;
        while k < self.n() {
            match self.text(k) {
                "{" | ";" => break,
                "where" if angle == 0 => break,
                "for" if angle == 0 => last = None,
                "<" => angle += 1,
                ">" => angle -= 1,
                "(" | "[" => k = self.pair(k).unwrap_or(k),
                "dyn" | "mut" | "const" => {}
                _ if angle == 0 && self.word(k) => last = Some(k),
                _ => {}
            }
            k += 1;
        }
        last
    }

    // ------------------------------------------------------------ TS/JS

    fn script_decls(&self) -> Vec<Decl> {
        let mut out = Vec::new();
        let mut classes = Vec::new();
        // the bodies of interfaces and type literals: a member there is a type
        let mut types = Vec::new();
        for i in 0..self.n() {
            if !self.word(i) || !self.starts(i, false) {
                continue;
            }
            let mut j = i;
            while matches!(
                self.text(j),
                "export" | "default" | "declare" | "abstract" | "async"
            ) && self.word(j + 1)
            {
                j += 1;
            }
            let k = j + 1;
            match self.text(j) {
                "function" => {
                    let k = if self.is(k, "*") { k + 1 } else { k };
                    if self.word(k) {
                        out.push(Decl::new(k, i, Shape::Block));
                    }
                }
                "class" if self.word(k) && !matches!(self.text(k), "extends" | "implements") => {
                    out.push(Decl::new(k, i, Shape::Block));
                    if let (_, Some(open)) = self.extent(i, Shape::Block) {
                        classes.push(open);
                    }
                }
                "interface" if self.word(k) => {
                    out.push(Decl::new(k, i, Shape::Block));
                    if let (_, Some(open)) = self.extent(i, Shape::Block) {
                        types.push(open);
                    }
                }
                "enum" | "namespace" | "module" if self.word(k) => {
                    out.push(Decl::new(k, i, Shape::Block))
                }
                "const" if self.is(k, "enum") && self.word(k + 1) => {
                    out.push(Decl::new(k + 1, i, Shape::Block))
                }
                "type" if self.word(k) && matches!(self.text(k + 1), "=" | "<") => {
                    out.push(Decl::new(k, i, Shape::Stmt));
                    let eq = if self.is(k + 1, "<") {
                        self.angle_close(k + 1) + 1
                    } else {
                        k + 1
                    };
                    if self.is(eq, "=") && self.is(eq + 1, "{") {
                        types.push(eq + 1);
                    }
                }
                "const" | "let" | "var" => self.bindings(i, k, &mut out),
                // a function assigned to a member: `a.b.name = function …`
                _ if self.word(j) && self.is(j + 1, ".") => {
                    let mut k = j;
                    while self.is(k + 1, ".") && self.word(k + 2) {
                        k += 2;
                    }
                    if self.is(k + 1, "=") && self.function_value(k + 2) {
                        out.push(Decl::fallback(k, i, Shape::Stmt));
                    }
                }
                _ => {}
            }
        }
        // a class member, or a method written anywhere an object has them
        for i in 0..self.n() {
            if !self.word(i) || !self.starts(i, true) {
                continue;
            }
            let Some(p) = self.parent(i).filter(|p| self.is(*p, "{")) else {
                continue;
            };
            let j = self.skip_modifiers(i);
            if !self.word(j) || NOT_A_METHOD.contains(&self.text(j)) {
                continue;
            }
            if classes.contains(&p) {
                if matches!(self.text(j + 1), "(" | "<" | ":" | "=" | "?" | "!" | ";")
                    || self.last_on_line(j)
                {
                    out.push(Decl::new(j, i, Shape::Member));
                }
            } else if self.method_shape(j) {
                out.push(Decl::new(j, i, Shape::Block));
            } else if self.is(j + 1, ":") && self.function_value(j + 2) && !self.within(p, &types) {
                // `name: (…) => …` or `name: function (…) {` — a method by
                // another spelling
                out.push(Decl::fallback(j, i, Shape::Member));
            }
        }
        out
    }

    /// Does a function or an arrow function begin at `k`?
    fn function_value(&self, k: usize) -> bool {
        let k = if self.is(k, "async") { k + 1 } else { k };
        if self.is(k, "function") || (self.word(k) && self.is(k + 1, "=>")) {
            return true;
        }
        let k = if self.is(k, "<") {
            self.angle_close(k) + 1
        } else {
            k
        };
        let Some(close) = self.is(k, "(").then(|| self.pair(k)).flatten() else {
            return false;
        };
        let mut m = close + 1;
        if self.is(m, "=>") {
            return true;
        }
        if !self.is(m, ":") {
            return false;
        }
        // a return type, then the arrow
        m += 1;
        while m < self.n() {
            match self.text(m) {
                "=>" => return true,
                "(" | "[" | "{" => m = self.pair(m).unwrap_or(m),
                "<" => m = self.angle_close(m),
                "," | ";" | "=" | ")" | "]" | "}" => return false,
                _ => {}
            }
            m += 1;
        }
        false
    }

    /// Is the bracket at `b`, or one around it, among `opens`?
    fn within(&self, b: usize, opens: &[usize]) -> bool {
        let mut q = Some(b);
        while let Some(o) = q {
            if opens.contains(&o) {
                return true;
            }
            q = self.parent(o);
        }
        false
    }

    /// The names a `const`/`let`/`var` statement declares, destructuring
    /// included; `from` is the first token after the keyword.
    fn bindings(&self, start: usize, from: usize, out: &mut Vec<Decl>) {
        let mut k = from;
        let mut first = true;
        loop {
            let at = if first { start } else { k };
            if self.word(k) {
                if matches!(self.text(k + 1), "=" | ":" | "," | ";" | "!") || self.last_on_line(k) {
                    out.push(Decl::new(k, at, Shape::Stmt));
                }
            } else if matches!(self.text(k), "{" | "[") {
                let close = self.pair(k).unwrap_or(k);
                for t in k + 1..close {
                    if self.word(t)
                        && matches!(self.text(t - 1), "{" | "[" | "," | "..." | ":")
                        && matches!(self.text(t + 1), "," | "}" | "]" | "=")
                    {
                        out.push(Decl::new(t, at, Shape::Stmt));
                    }
                }
            } else {
                return;
            }
            match self.next_declarator(k) {
                Some(next) => {
                    k = next;
                    first = false;
                }
                None => return,
            }
        }
    }

    fn next_declarator(&self, from: usize) -> Option<usize> {
        let mut t = from;
        while t < self.n() {
            match self.text(t) {
                "(" | "[" | "{" => t = self.pair(t)?,
                ")" | "]" | "}" | ";" => return None,
                "," => return Some(t + 1),
                _ => {}
            }
            if self.last_on_line(t) && !self.trailing(t) && !self.leading(t + 1) {
                return None;
            }
            t += 1;
        }
        None
    }

    /// `name<…>(…) {` or `name(…): T {` — a method definition.
    fn method_shape(&self, j: usize) -> bool {
        let mut k = j + 1;
        if self.is(k, "<") {
            k = self.angle_close(k) + 1;
        }
        if !self.is(k, "(") {
            return false;
        }
        let Some(close) = self.pair(k) else {
            return false;
        };
        k = close + 1;
        if self.is(k, "{") {
            return true;
        }
        if !self.is(k, ":") {
            return false;
        }
        let mut angle = 0i64;
        k += 1;
        while k < self.n() {
            match self.text(k) {
                "{" if angle == 0 => return true,
                "(" | "[" | "{" => k = self.pair(k).unwrap_or(k),
                "<" => angle += 1,
                ">" => angle -= 1,
                ";" | "=" | "=>" | "," | "}" | ")" | "]" => return false,
                _ => {}
            }
            k += 1;
        }
        false
    }

    // ------------------------------------------------------------ Go

    fn go_decls(&self) -> Vec<Decl> {
        let mut out = Vec::new();
        for i in 0..self.n() {
            match self.text(i) {
                "func" if self.word(i + 1) => out.push(Decl::new(i + 1, i, Shape::Block)),
                "func" if self.is(i + 1, "(") => {
                    let Some(close) = self.pair(i + 1) else {
                        continue;
                    };
                    let name = close + 1;
                    if self.word(name) && matches!(self.text(name + 1), "(" | "[") {
                        out.push(Decl {
                            owner: self.receiver(i + 1, close),
                            ..Decl::new(name, i, Shape::Block)
                        });
                    }
                }
                "type" | "var" | "const" if self.is(i + 1, "(") => {
                    let Some(close) = self.pair(i + 1) else {
                        continue;
                    };
                    for t in i + 2..close {
                        if self.parent(t) == Some(i + 1) && self.word(t) && self.first_on_line(t) {
                            self.go_names(t, t, &mut out);
                        }
                    }
                }
                "type" | "var" | "const" if self.word(i + 1) => self.go_names(i + 1, i, &mut out),
                _ => {}
            }
        }
        out
    }

    fn go_type(&self, d: &Decl) -> bool {
        self.is(d.start, "type")
            || self
                .parent(d.name)
                .is_some_and(|p| p > 0 && self.is(p - 1, "type"))
    }

    fn go_names(&self, first: usize, start: usize, out: &mut Vec<Decl>) {
        let mut k = first;
        out.push(Decl::new(k, start, Shape::Stmt));
        while self.is(k + 1, ",") && self.word(k + 2) {
            k += 2;
            out.push(Decl::new(k, start, Shape::Stmt));
        }
    }

    /// `(r *T)`, `(r T[K])`, `(T)`: the receiver's type name.
    fn receiver(&self, open: usize, close: usize) -> Option<usize> {
        let words: Vec<usize> = (open + 1..close).filter(|t| self.word(*t)).collect();
        match words.as_slice() {
            [only] => Some(*only),
            [_, ty, ..] => Some(*ty),
            [] => None,
        }
    }

    // ------------------------------------------------------------ others

    /// The generic rule: a name preceded on its line only by declaration
    /// words and followed by `(`, `{`, `=`, `:` or `<`.
    fn brace_decls(&self) -> Vec<Decl> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.n() {
            let line = self.line(i);
            let mut j = i;
            while j < self.n() && self.line(j) == line {
                j += 1;
            }
            if !NOT_A_DECLARATION.contains(&self.text(i)) {
                let mut words = 0;
                for k in i..j {
                    if words > 0 && self.word(k) && self.brace_follows(k) {
                        out.push(Decl::new(k, i, Shape::Block));
                    }
                    // what a class extends or implements is not declared here
                    if matches!(self.text(k), "extends" | "implements") {
                        break;
                    }
                    if self.word(k) {
                        words += 1;
                    } else if !matches!(self.text(k), "<" | ">" | "*" | "&" | "[" | "]" | ",") {
                        break;
                    }
                }
            }
            i = j;
        }
        out
    }

    /// `<` declares only when it opens the name's own parameters: in
    /// `List<String> names =` the name declared is `names`. A class that
    /// `extends` or `implements` declares as one followed by `{`.
    fn brace_follows(&self, k: usize) -> bool {
        match self.text(k + 1) {
            "(" | "{" | "=" | ":" | "extends" | "implements" => true,
            "<" => {
                let c = self.angle_close(k + 1);
                !(self.word(c + 1)
                    && matches!(self.text(c + 2), "(" | "=" | ";" | "," | ")" | "{" | ":"))
            }
            _ => false,
        }
    }

    // ------------------------------------------------------------ queries

    fn decls(&self) -> Decls {
        let mut impls = Vec::new();
        let mut all = match self.fam {
            Family::Rust => self.rust_decls(&mut impls),
            Family::Script => self.script_decls(),
            Family::Python => self.python_decls(),
            Family::Go => self.go_decls(),
            Family::Brace => self.brace_decls(),
        };
        all.sort_by_key(|d| d.name);
        all.dedup_by_key(|d| d.name);
        Decls { all, impls }
    }

    /// Python declares at module level only by `def`, `class` or an
    /// assignment at column 0; everything else may be found at any depth.
    fn top(&self, d: &Decl) -> bool {
        d.shape != Shape::Logical || indent(self.line_text(self.line(d.start))) == 0
    }

    /// The declarations directly inside a region: what its body declares,
    /// and the member headers among its direct children.
    fn members(&self, r: &Region, all: &[Decl]) -> Vec<Decl> {
        if self.fam == Family::Python {
            let Some(body) = (r.first + 1..=r.last).find(|l| {
                let text = self.line_text(*l).trim_start();
                !self.is_cont(*l) && !text.is_empty() && !text.starts_with('#')
            }) else {
                return Vec::new();
            };
            let want = indent(self.line_text(body));
            return all
                .iter()
                .filter(|d| {
                    let l = self.line(d.start);
                    l > r.first && l <= r.last && indent(self.line_text(l)) == want
                })
                .copied()
                .collect();
        }
        let Some(body) = r.body else {
            return Vec::new();
        };
        let mut out: Vec<Decl> = all
            .iter()
            .filter(|d| self.parent(d.name) == Some(body))
            .copied()
            .collect();
        let close = self.pair(body).unwrap_or(self.n());
        for i in body + 1..close {
            if self.parent(i) != Some(body) || !self.word(i) || !self.starts(i, true) {
                continue;
            }
            let j = self.skip_modifiers(i);
            if self.parent(j) != Some(body) || !self.word(j) || out.iter().any(|d| d.name == j) {
                continue;
            }
            let follows = matches!(
                self.text(j + 1),
                "(" | "<" | ":" | "=" | "?" | "!" | ";" | "," | "{" | "}"
            ) || self.last_on_line(j)
                || (self.fam == Family::Go && self.word(j + 1));
            if follows {
                out.push(Decl::new(j, i, Shape::Member));
            }
        }
        out
    }

    /// Tokens inside an import: a name there is neither a declaration nor a
    /// use.
    fn imports(&self) -> Vec<bool> {
        let mut mark = vec![false; self.n()];
        for i in 0..self.n() {
            let opens = match self.fam {
                Family::Script => {
                    self.starts(i, false)
                        && ((self.is(i, "import") && !matches!(self.text(i + 1), "(" | "."))
                            || (self.is(i, "export")
                                && (self.is(i + 1, "*")
                                    || (self.is(i + 1, "{")
                                        && self
                                            .pair(i + 1)
                                            .is_some_and(|c| self.is(c + 1, "from"))))))
                }
                Family::Rust => {
                    self.is(i, "use") || (self.is(i, "extern") && self.is(i + 1, "crate"))
                }
                Family::Go => self.is(i, "import"),
                Family::Python => {
                    self.logical_start(i) && matches!(self.text(i), "import" | "from")
                }
                Family::Brace => {
                    self.first_on_line(i)
                        && (matches!(self.text(i), "import" | "package" | "use")
                            || (self.is(i, "using")
                                && self.word(i + 1)
                                && matches!(self.text(i + 2), "." | ";" | "="))
                            || (self.is(i, "#")
                                && matches!(self.text(i + 1), "include" | "import")))
                }
            };
            if !opens {
                continue;
            }
            let end = if self.fam == Family::Python {
                self.logical_end(i)
            } else {
                self.extent(i, Shape::Stmt).0
            };
            for m in mark.iter_mut().take(end + 1).skip(i) {
                *m = true;
            }
        }
        mark
    }

    /// The lines where `name` occurs in code, outside imports and outside
    /// the names of declarations.
    fn use_lines(&self, name: &str, decls: &Decls) -> Vec<usize> {
        let imports = self.imports();
        let mut out: Vec<usize> = self
            .toks
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                t.kind == Kind::Word
                    && t.text == name
                    && !imports.get(*i).copied().unwrap_or(false)
                    && !decls.all.iter().any(|d| d.name == *i)
            })
            .map(|(_, t)| t.line + 1)
            .collect();
        out.dedup();
        out
    }
}

fn lines_phrase(lines: &[usize]) -> String {
    match lines {
        [one] => format!("line {one}"),
        _ => format!(
            "lines {}",
            lines
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn segments(symbol: &str) -> Vec<&str> {
    let sep = if symbol.contains("::") { "::" } else { "." };
    symbol
        .split(sep)
        .map(|s| s.trim().trim_start_matches('#'))
        .filter(|s| !s.is_empty())
        .collect()
}

/// The lines (1-based, inclusive) of the declaration `symbol` names in the
/// file at `path` (D-106). `Type.member`, `Type::member` and
/// `Outer.Inner.name` resolve the owner first, then the member inside it; a
/// Rust type's owners are its item and every `impl` block for it. Every
/// refusal names the lines behind it (R-114).
pub fn resolve(source: &str, path: &str, symbol: &str) -> Result<(usize, usize), String> {
    let fam = family(path);
    let blanked =
        matches!(extension(path).as_str(), "vue" | "svelte").then(|| script_blocks(source));
    let src = Src::new(blanked.as_deref().unwrap_or(source), source, fam);
    let decls = src.decls();
    let named = |list: &[Decl], name: &str| -> Vec<Decl> {
        list.iter()
            .filter(|d| src.text(d.name) == name)
            .copied()
            .collect()
    };
    let name_lines = |list: &[Decl]| -> String {
        let mut lines: Vec<usize> = list.iter().map(|d| src.line(d.name) + 1).collect();
        lines.sort_unstable();
        lines.dedup();
        lines_phrase(&lines)
    };
    let segs = segments(symbol);
    let Some((head, rest)) = segs.split_first() else {
        return Err(format!("symbol `{symbol}` not found in `{path}`"));
    };
    let mut scope: Vec<Decl> = decls
        .all
        .iter()
        .filter(|d| src.top(d) && src.text(d.name) == *head)
        .copied()
        .collect();
    if scope.iter().any(|d| !d.fallback) {
        scope.retain(|d| !d.fallback);
    }
    // an `impl` stands for a type declared in another file
    if scope.is_empty() && fam == Family::Rust {
        scope = named(&decls.impls, head);
    }
    let go_methods = fam == Family::Go
        && !rest.is_empty()
        && decls
            .all
            .iter()
            .any(|d| d.owner.is_some_and(|o| src.text(o) == *head));
    if scope.is_empty() && !go_methods {
        let uses = src.use_lines(head, &decls);
        return Err(if uses.is_empty() {
            format!("symbol `{symbol}` not found in `{path}`")
        } else {
            format!(
                "symbol `{symbol}` is not declared in `{path}`: it occurs only outside \
                 declarations, at {} — pin the file that declares it, or this whole file",
                lines_phrase(&uses)
            )
        });
    }
    let sep = if fam == Family::Rust { "::" } else { "." };
    let mut owner: &str = head;
    for seg in rest {
        // a Go method hangs on a type, never on a variable of the same name
        let mut owners: Vec<Decl> = scope
            .iter()
            .filter(|d| !decls.impls.contains(d) && (fam != Family::Go || src.go_type(d)))
            .copied()
            .collect();
        if owners.len() > 1 {
            return Err(format!(
                "symbol `{symbol}` is ambiguous in `{path}`: `{owner}` is declared at {} — pin \
                 the whole file",
                name_lines(&owners)
            ));
        }
        if fam == Family::Rust {
            owners.extend(named(&decls.impls, owner));
        }
        let mut next: Vec<Decl> = Vec::new();
        for d in &owners {
            next.extend(named(&src.members(&src.region(d), &decls.all), seg));
        }
        if fam == Family::Go {
            next.extend(
                decls
                    .all
                    .iter()
                    .filter(|d| {
                        d.owner.is_some_and(|o| src.text(o) == owner) && src.text(d.name) == *seg
                    })
                    .copied(),
            );
        }
        next.sort_by_key(|d| d.name);
        next.dedup_by_key(|d| d.name);
        if next.is_empty() {
            let at = if owners.is_empty() {
                String::new()
            } else {
                format!(" (at {})", name_lines(&owners))
            };
            return Err(format!(
                "symbol `{symbol}` not found in `{path}`: `{owner}`{at} declares no `{seg}`"
            ));
        }
        owner = seg;
        scope = next;
    }
    match scope.as_slice() {
        [one] => {
            let r = src.region(one);
            Ok((r.first + 1, r.last + 1))
        }
        many => Err(format!(
            "symbol `{symbol}` is ambiguous in `{path}`: declared at {} — narrow it with a \
             member path (`Owner{sep}{owner}`) or pin the whole file",
            name_lines(many)
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const RUST_HEADER: &str = r##"use crate::refresh;

#[inline]
pub(crate) async fn refresh<T>(
    t: T,
) -> u32
where
    T: Copy,
{
    if t { "}" } else { 0 }
}
fn other() { refresh(1); }
"##;

    const RUST_ITEMS: &str = r##"pub struct Point {
    x: i32,
}
pub struct Unit;
enum Shape { Dot, Line }
union Bits { a: u32 }
pub trait Area {
    fn area(&self) -> f64;
}
pub type Grid =
    Vec<Point>;
pub const LIMIT: [u32; 2] = [
    1, 2,
];
static mut COUNT: u32 = 0;
mod inner;
macro_rules! square {
    ($x:expr) => { $x * $x };
}
extern "C" fn c_entry() {}
"##;

    const RUST_LEXEMES: &str = r##"pub fn parse<'a>(s: &'a str) -> Option<char> {
    let open = '{';
    let raw = r#"}"#;
    /* } */
    s.chars().find(|c| *c == open)
}
"##;

    const RUST_LEDGER: &str = r##"pub struct Ledger {
    total: u64,
}

impl Ledger {
    pub fn new() -> Self {
        Ledger { total: 0 }
    }
}

impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}

impl Ledger {
    pub fn transfer(&mut self) {
        self.total += 1;
    }
}
fn transfer() {}
"##;

    const RUST_IMPL_ONLY: &str = r##"impl Widget {
    fn draw(&self) {}
}
"##;

    const TS_USES: &str = r##"import { X } from './x';

export function check(spec: Spec, y: string) {
  if (spec.label !== undefined && X.test(y)) {
    return true;
  }
  for (let i = 0; i < Math.min(v.length, X); i++) {
    log(`X is ${i}`);
  }
  return false;
}
"##;

    const TS_ONE_USE: &str = r##"export function run(y: string) {
  if (spec.label !== undefined && Pattern.test(y)) {
    return 1;
  }
}
"##;

    const TS_TEMPLATE: &str = r##"export function stream(id: string) {
  return id;
}
logger.info(`stream {${id}} opened`)
"##;

    const TS_SHAPES: &str = r##"export function f(
  a: T,
) {
  return a;
}
export type T =
  | A
  | B;
export type U =
  | A
  | B
export const isX = (a) => { return a.kind === 'x'; };
export const isY = (a: A): a is Y =>
  a.kind === 'y';
const CFG = {
  a: 1,
};
export const LIMITS = { max: 3 };
"##;

    const TS_CLASS: &str = r##"export abstract class Store<T> extends Base<{ id: string }> {
  private readonly items: T[] = [];
  static #count = 0;
  constructor(private api: Api) {
    super();
  }
  async save(item: T): Promise<void> {
    this.items.push(item);
  }
  get size(): number {
    return this.items.length;
  }
  abstract load(id: string): Promise<T>;
  label = 'store'
}
export function save(x: number) {
  return x;
}
"##;

    const TS_OVERLOADS: &str = r##"export function parse(a: string): A;
export function parse(a: number): B;
export function parse(a: any) {
  return a;
}
"##;

    const TS_KINDS: &str = r##"export interface Props {
  label: string
  onClick(): void
}
export const enum Mode { A, B }
declare namespace Lib {
  function init(): void;
}
declare module Shapes {
  export const SIDES = 4;
}
enum Color {
  Red,
  Green = 'g',
}
"##;

    const JS_KINDS: &str = r##"export default class Widget {}
function* gen() {
  yield 1;
}
export const { alpha, beta: b } = load();
let counter = 0, total = 1;
var legacy = {
  key: 1,
}
const api = {
  fetch() {
    return 1
  },
}
"##;

    const JS_FUNCTION_VALUES: &str = r##"export const useStore = defineStore('s', {
  getters: {
    badges: (state): Badge[] => {
      return state.list;
    },
    theme: state => {
      return state.t;
    },
    count: (state) => state.n,
    legacy: function (state) {
      return state.l;
    },
  },
});
editorMethods.measure = function (): number[] {
  return [];
};
interface Shape {
  draw: (ctx: Ctx) => void;
}
type Hooks = {
  onDone: () => void;
};
"##;

    const JS_FUNCTION_AND_VALUE: &str = r##"export function getParameter(name: string): string {
  return name;
}
const mock = {
  stub: () => 1,
  fns: { getParameter: () => String(1) },
};
"##;

    const VUE_SFC: &str = r##"<template>
  <div @click="save">{{ label }}</div>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const label = ref('save')
function save() {
  label.value = 'saved'
}
</script>

<style>
.save { color: red }
</style>
"##;

    const SVELTE: &str = r##"<script lang="ts">
  export let name: string;
  function greet() {
    return `hi ${name}`;
  }
</script>

<h1 on:click={greet}>{name}</h1>
"##;

    const PY: &str = r##"import os
from typing import (
    Dict,
)

LIMIT = 3
TABLE: Dict[str, int] = {
    "a": 1,
}

@decorator
async def fetch(
    url: str,
) -> str:
    """Fetch the url."""
    return url

class Ledger:
    RATE = 2

    def transfer(self):
        text = """
not code: def transfer
"""
        return text

def transfer():
    pass
"##;

    const PYI: &str = "def stub(x: int) -> int: ...\n";

    const GO: &str = r##"package store

import (
    "fmt"
)

const (
    Small = iota
    Large
)

var Default = Config{
    Size: Small,
}

type Config struct {
    Size int
}

type ID string

func New(size int) *Config {
    return &Config{Size: size}
}

func (c *Config) Grow(n int) {
    c.Size += n
    fmt.Println("Grow {")
}
"##;

    const JAVA: &str = r##"package app;
import java.util.List;

public class Service {
    private final List<String> names = List.of(
        "a");

    @Override
    public String run(int count)
        throws Exception
    {
        if (count > 0) { return names.get(0); }
        return run(count - 1);
    }
}
"##;

    const KOTLIN: &str = r##"class Greeter(val name: String) {
    fun greet(): String {
        return "Hello, $name {"
    }
}
val DEFAULT = Greeter("x")
"##;

    const C: &str = r##"#include <stdio.h>
static int count = 0;
int add(int a, int b)
{
    return a + b;
}
"##;

    const GO_SHAPES: &str = r##"package p

func (p *pipe) Done() <-chan struct{} {
    return p.done
}

func linked(*Transport, *Request) (*Response, error)

type body struct {
    xx int
    as int
}

func (b *body) Read(p []byte) (int, error) {
    var body = b.xx
    return body, nil
}
"##;

    const PY_COMMENTS: &str = r##"class Reader:  # a reader
               # of streams

    def read(self):
        return 1

##  def old(self):
##      return 0

    def close(self):
        pass
"##;

    const TS_RETURN_TYPES: &str = r##"export function requireParams(): { name: string; hash: string } {
  return { name: '', hash: '' };
}
export function reDefault(html: string): {
  html: string;
  changed: boolean;
} {
  return { html, changed: false };
}
function isMulti(o: Options): o is Options & {
  updates: string[];
} {
  return !!o.updates;
}
"##;

    const JAVA_EXTENDS: &str = r##"public class Admin extends User implements Audited {
    public void audit() {}
}
"##;

    const PHP: &str = r##"<?php
namespace App;

use App\Models\Account as Authenticatable;

class User extends Authenticatable
{
    public function __construct(private readonly Repo $repo) {}

    public function name(): string
    {
        return $this->repo->name();
    }
}
"##;

    /// `(file, source, symbol) → (first line, last line) | a substring of the
    /// refusal`.
    #[allow(clippy::type_complexity)]
    const CASES: &[(&str, &str, &str, Result<(usize, usize), &str>)] = &[
        // Rust
        ("a.rs", RUST_HEADER, "refresh", Ok((4, 11))),
        ("a.rs", RUST_ITEMS, "Point", Ok((1, 3))),
        ("a.rs", RUST_ITEMS, "Unit", Ok((4, 4))),
        ("a.rs", RUST_ITEMS, "Shape", Ok((5, 5))),
        ("a.rs", RUST_ITEMS, "Bits", Ok((6, 6))),
        ("a.rs", RUST_ITEMS, "Area", Ok((7, 9))),
        ("a.rs", RUST_ITEMS, "Area::area", Ok((8, 8))),
        ("a.rs", RUST_ITEMS, "Grid", Ok((10, 11))),
        ("a.rs", RUST_ITEMS, "LIMIT", Ok((12, 14))),
        ("a.rs", RUST_ITEMS, "COUNT", Ok((15, 15))),
        ("a.rs", RUST_ITEMS, "inner", Ok((16, 16))),
        ("a.rs", RUST_ITEMS, "square", Ok((17, 19))),
        ("a.rs", RUST_ITEMS, "c_entry", Ok((20, 20))),
        ("a.rs", RUST_LEXEMES, "parse", Ok((1, 6))),
        ("a.rs", RUST_LEDGER, "Ledger", Ok((1, 3))),
        ("a.rs", RUST_LEDGER, "Ledger::transfer", Ok((18, 20))),
        ("a.rs", RUST_LEDGER, "Ledger::default", Ok((12, 14))),
        ("a.rs", RUST_LEDGER, "Ledger::total", Ok((2, 2))),
        ("a.rs", RUST_LEDGER, "Ledger::missing", Err("not found")),
        ("a.rs", RUST_LEDGER, "transfer", Err("lines 18, 22")),
        ("a.rs", RUST_IMPL_ONLY, "Widget", Ok((1, 3))),
        // TS/JS
        ("a.ts", TS_USES, "check", Ok((3, 11))),
        (
            "a.ts",
            TS_USES,
            "X",
            Err("only outside declarations, at lines 4, 7"),
        ),
        (
            "a.ts",
            TS_ONE_USE,
            "Pattern",
            Err("only outside declarations, at line 2"),
        ),
        ("a.ts", TS_TEMPLATE, "stream", Ok((1, 3))),
        ("a.ts", TS_SHAPES, "f", Ok((1, 5))),
        ("a.ts", TS_SHAPES, "T", Ok((6, 8))),
        ("a.ts", TS_SHAPES, "U", Ok((9, 11))),
        ("a.ts", TS_SHAPES, "isX", Ok((12, 12))),
        ("a.ts", TS_SHAPES, "isY", Ok((13, 14))),
        ("a.ts", TS_SHAPES, "CFG", Ok((15, 17))),
        ("a.ts", TS_SHAPES, "CFG.a", Ok((16, 16))),
        ("a.ts", TS_SHAPES, "LIMITS", Ok((18, 18))),
        ("a.ts", TS_CLASS, "Store", Ok((1, 15))),
        ("a.ts", TS_CLASS, "Store.save", Ok((7, 9))),
        ("a.ts", TS_CLASS, "Store.size", Ok((10, 12))),
        ("a.ts", TS_CLASS, "Store.items", Ok((2, 2))),
        ("a.ts", TS_CLASS, "Store.load", Ok((13, 13))),
        ("a.ts", TS_CLASS, "Store.label", Ok((14, 14))),
        ("a.ts", TS_CLASS, "save", Err("lines 7, 16")),
        ("a.ts", TS_OVERLOADS, "parse", Err("lines 1, 2, 3")),
        ("a.ts", TS_KINDS, "Props", Ok((1, 4))),
        ("a.ts", TS_KINDS, "Props.label", Ok((2, 2))),
        ("a.ts", TS_KINDS, "Props.onClick", Ok((3, 3))),
        ("a.ts", TS_KINDS, "Mode", Ok((5, 5))),
        ("a.ts", TS_KINDS, "Lib", Ok((6, 8))),
        ("a.ts", TS_KINDS, "Lib.init", Ok((7, 7))),
        ("a.ts", TS_KINDS, "Shapes.SIDES", Ok((10, 10))),
        ("a.ts", TS_KINDS, "Color", Ok((12, 15))),
        ("a.ts", TS_KINDS, "Color.Green", Ok((14, 14))),
        ("a.tsx", TS_KINDS, "Lib.init", Ok((7, 7))),
        ("a.mts", TS_SHAPES, "T", Ok((6, 8))),
        ("a.cts", TS_SHAPES, "isY", Ok((13, 14))),
        ("a.jsx", JS_KINDS, "Widget", Ok((1, 1))),
        ("a.js", JS_KINDS, "gen", Ok((2, 4))),
        ("a.mjs", JS_KINDS, "alpha", Ok((5, 5))),
        ("a.mjs", JS_KINDS, "b", Ok((5, 5))),
        ("a.cjs", JS_KINDS, "total", Ok((6, 6))),
        ("a.js", JS_KINDS, "legacy", Ok((7, 9))),
        ("a.js", JS_KINDS, "api.fetch", Ok((11, 13))),
        ("a.js", JS_KINDS, "fetch", Ok((11, 13))),
        ("a.ts", JS_FUNCTION_VALUES, "badges", Ok((3, 5))),
        ("a.ts", JS_FUNCTION_VALUES, "theme", Ok((6, 8))),
        ("a.ts", JS_FUNCTION_VALUES, "count", Ok((9, 9))),
        ("a.ts", JS_FUNCTION_VALUES, "legacy", Ok((10, 12))),
        ("a.ts", JS_FUNCTION_VALUES, "measure", Ok((15, 17))),
        (
            "a.ts",
            JS_FUNCTION_VALUES,
            "draw",
            Err("only outside declarations"),
        ),
        (
            "a.ts",
            JS_FUNCTION_VALUES,
            "onDone",
            Err("only outside declarations"),
        ),
        ("a.ts", JS_FUNCTION_VALUES, "Shape.draw", Ok((19, 19))),
        ("a.ts", JS_FUNCTION_AND_VALUE, "getParameter", Ok((1, 3))),
        ("a.ts", JS_FUNCTION_AND_VALUE, "stub", Ok((5, 5))),
        ("Panel.vue", VUE_SFC, "save", Ok((9, 11))),
        ("Panel.vue", VUE_SFC, "label", Ok((8, 8))),
        ("Hello.svelte", SVELTE, "greet", Ok((3, 5))),
        ("Hello.svelte", SVELTE, "name", Ok((2, 2))),
        // Python
        ("a.py", PY, "LIMIT", Ok((6, 6))),
        ("a.py", PY, "TABLE", Ok((7, 9))),
        ("a.py", PY, "fetch", Ok((12, 16))),
        ("a.py", PY, "Ledger", Ok((18, 25))),
        ("a.py", PY, "Ledger.transfer", Ok((21, 25))),
        ("a.py", PY, "Ledger.RATE", Ok((19, 19))),
        ("a.py", PY, "transfer", Err("lines 21, 27")),
        (
            "a.py",
            PY,
            "Dict",
            Err("only outside declarations, at line 7"),
        ),
        ("a.py", PY, "os", Err("not found")),
        ("a.pyi", PYI, "stub", Ok((1, 1))),
        // Go
        ("a.go", GO, "Small", Ok((8, 8))),
        ("a.go", GO, "Large", Ok((9, 9))),
        ("a.go", GO, "Default", Ok((12, 14))),
        ("a.go", GO, "Config", Ok((16, 18))),
        ("a.go", GO, "Config.Size", Ok((17, 17))),
        ("a.go", GO, "ID", Ok((20, 20))),
        ("a.go", GO, "New", Ok((22, 24))),
        ("a.go", GO, "Grow", Ok((26, 29))),
        ("a.go", GO, "Config.Grow", Ok((26, 29))),
        // every other brace language
        ("Service.java", JAVA, "Service", Ok((4, 15))),
        ("Service.java", JAVA, "run", Ok((9, 14))),
        ("Service.java", JAVA, "Service.run", Ok((9, 14))),
        ("Service.java", JAVA, "names", Ok((5, 6))),
        (
            "Service.java",
            JAVA,
            "List",
            Err("only outside declarations, at line 5"),
        ),
        ("a.kt", KOTLIN, "Greeter", Ok((1, 5))),
        ("a.kt", KOTLIN, "Greeter.greet", Ok((2, 4))),
        ("a.kt", KOTLIN, "DEFAULT", Ok((6, 6))),
        ("a.c", C, "add", Ok((3, 6))),
        ("a.c", C, "count", Ok((2, 2))),
        // shapes measured against the languages' own parsers
        ("a.go", GO_SHAPES, "pipe.Done", Ok((3, 5))),
        ("a.go", GO_SHAPES, "linked", Ok((7, 7))),
        ("a.go", GO_SHAPES, "body.xx", Ok((10, 10))),
        ("a.go", GO_SHAPES, "body.Read", Ok((14, 17))),
        ("a.go", GO_SHAPES, "body", Err("lines 9, 15")),
        ("a.py", PY_COMMENTS, "Reader", Ok((1, 11))),
        ("a.py", PY_COMMENTS, "Reader.close", Ok((10, 11))),
        ("a.ts", TS_RETURN_TYPES, "requireParams", Ok((1, 3))),
        ("a.ts", TS_RETURN_TYPES, "reDefault", Ok((4, 9))),
        ("a.ts", TS_RETURN_TYPES, "isMulti", Ok((10, 14))),
        ("Admin.java", JAVA_EXTENDS, "Admin", Ok((1, 3))),
        ("User.php", PHP, "User", Ok((6, 14))),
        ("User.php", PHP, "User.name", Ok((10, 13))),
        (
            "User.php",
            PHP,
            "Authenticatable",
            Err("only outside declarations, at line 6 "),
        ),
    ];

    #[test]
    fn a_symbol_resolves_to_its_declaration_in_every_family() {
        let mut wrong = Vec::new();
        for (file, src, sym, want) in CASES {
            let got = resolve(src, file, sym);
            let ok = match (want, &got) {
                (Ok(w), Ok(g)) => w == g,
                (Err(w), Err(g)) => g.contains(w),
                _ => false,
            };
            if !ok {
                wrong.push(format!("{file} {sym}: want {want:?}, got {got:?}"));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of {} rows wrong:\n{}",
            wrong.len(),
            CASES.len(),
            wrong.join("\n")
        );
    }

    #[test]
    fn impl_in_a_signature_is_no_impl_block() {
        let src = "pub struct Config {\n    v: u32,\n}\n\npub fn from_path(p: impl AsRef<str>) -> Config {\n    Config { v: 0 }\n}\n\npub fn all() -> impl Iterator<Item = Config> {\n    std::iter::empty()\n}\n\nimpl Config {\n    pub fn new() -> Self {\n        Config { v: 1 }\n    }\n}\n";
        assert_eq!(resolve(src, "a.rs", "Config"), Ok((1, 3)));
        assert_eq!(resolve(src, "a.rs", "Config::new"), Ok((14, 16)));
        assert!(resolve(src, "a.rs", "AsRef").is_err());
        assert!(resolve(src, "a.rs", "Iterator").is_err());
    }
}
