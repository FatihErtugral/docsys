//! `docsys consume add|discover` — a consumer's provider list lives in its
//! own `.docmeta.yml` and nowhere else (D-075): no registry outside the
//! repository, nothing written into a provider. `add` appends one provider
//! from a path or a git URL, reading the provider's `namespace:`;
//! `discover` lists the docsys trees under a directory as candidates and
//! writes nothing — the plan is shown, the person runs `add`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::tree::DocTree;

/// A directory or repository name as a local identifier: lowercase, digits,
/// single hyphens (R-060).
pub fn local_id_of(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub ns: String,
    pub path: PathBuf,
    pub profile: String,
    /// already in this tree's `consume:` list
    pub already: bool,
}

/// The docs root inside a checkout: the directory itself when it carries
/// `.docmeta.yml` (a root-level base), else its `docs/`.
fn docs_root_of(dir: &Path) -> Option<(PathBuf, &'static str)> {
    if dir.join(".docmeta.yml").is_file() {
        Some((dir.to_path_buf(), ""))
    } else if dir.join("docs/.docmeta.yml").is_file() {
        Some((dir.join("docs"), "docs"))
    } else {
        None
    }
}

fn namespace_of(provider_root: &Path, dir: &Path) -> Result<(String, String), String> {
    let tree = DocTree::load(provider_root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", provider_root.display()));
    }
    let profile = tree
        .docmeta_str("profile")
        .unwrap_or("project")
        .trim()
        .to_string();
    let ns = tree
        .docmeta_str("namespace")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            local_id_of(
                &dir.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
        });
    Ok((ns, profile))
}

/// The namespaces this tree already consumes (bare or `ns=location`).
fn consumed(tree: &DocTree) -> Vec<String> {
    tree.docmeta_list("consume")
        .iter()
        .map(|e| {
            e.split_once('=')
                .map_or(e.trim(), |(n, _)| n.trim())
                .to_string()
        })
        .collect()
}

/// `.docmeta.yml` with one more `consume:` entry, whatever shape the list
/// has (inline, block, absent).
fn with_entry(text: &str, entry: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let Some(at) = lines.iter().position(|l| l.starts_with("consume:")) else {
        let mut out = text.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("consume: [{entry}]\n"));
        return out;
    };
    let mut out: Vec<String> = lines.iter().map(|l| (*l).to_string()).collect();
    let line = lines.get(at).copied().unwrap_or_default();
    let rest = line.trim_start_matches("consume:").trim();
    if rest.starts_with('[') {
        let inner = rest.trim_start_matches('[').trim_end_matches(']').trim();
        let new = if inner.is_empty() {
            format!("consume: [{entry}]")
        } else {
            format!("consume: [{inner}, {entry}]")
        };
        if let Some(slot) = out.get_mut(at) {
            *slot = new;
        }
    } else {
        // block list: after the last `  - ` item of this key
        let mut end = at;
        for (i, l) in lines.iter().enumerate().skip(at + 1) {
            if l.starts_with("  - ") {
                end = i;
            } else if !l.trim().is_empty() {
                break;
            }
        }
        out.insert(end + 1, format!("  - {entry}"));
    }
    let mut joined = out.join("\n");
    joined.push('\n');
    joined
}

/// `docsys consume add <path|git-url>[#subdir] [--as <ns>]`.
pub fn add(root: &Path, target: &str, as_ns: Option<&str>) -> Result<String, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let (loc, sub) = match target.split_once('#') {
        Some((l, s)) => (l.trim().to_string(), s.trim().to_string()),
        None => (target.trim().to_string(), String::new()),
    };
    let (ns, location, note) = if crate::export::is_git_url(&loc) {
        let name = loc
            .trim_end_matches('/')
            .rsplit(['/', ':'])
            .next()
            .unwrap_or(&loc)
            .trim_end_matches(".git");
        let ns = as_ns.map_or_else(|| local_id_of(name), |s| s.to_string());
        let sub = if sub.is_empty() {
            "docs".to_string()
        } else {
            sub
        };
        (ns, format!("{loc}#{sub}"), "git".to_string())
    } else {
        let dir = Path::new(&loc);
        let dir = dir
            .canonicalize()
            .map_err(|_| format!("`{loc}` is not a directory that exists"))?;
        let (provider_root, sub) = if sub.is_empty() {
            docs_root_of(&dir).ok_or_else(|| {
                format!(
                    "no docsys tree at `{}` — neither `.docmeta.yml` nor `docs/.docmeta.yml`",
                    dir.display()
                )
            })?
        } else {
            (dir.join(&sub), "")
        };
        let sub = if sub.is_empty() && provider_root != dir {
            provider_root
                .strip_prefix(&dir)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default()
        } else {
            sub.to_string()
        };
        let (found, profile) = namespace_of(&provider_root, &dir)?;
        let ns = as_ns.map_or(found, str::to_string);
        let location = if sub.is_empty() {
            dir.to_string_lossy().replace('\\', "/")
        } else {
            format!("{}#{sub}", dir.to_string_lossy().replace('\\', "/"))
        };
        (ns, location, profile)
    };
    if !crate::model::is_local_id(&ns) {
        return Err(format!(
            "`{ns}` is not a local-id — name it with `--as <ns>`"
        ));
    }
    if consumed(&tree).iter().any(|c| c == &ns) {
        return Err(format!("`{ns}` is already in this tree's `consume:` list"));
    }
    // the template form when the base already names this location
    let entry = match tree.docmeta_str("consume_base") {
        Some(base) if base.replace("{ns}", &ns) == location => ns.clone(),
        _ => format!("{ns}={location}"),
    };
    let dm = root.join(".docmeta.yml");
    let text = fs::read_to_string(&dm).map_err(|e| e.to_string())?;
    fs::write(&dm, with_entry(&text, &entry)).map_err(|e| e.to_string())?;
    Ok(format!(
        "consume: {ns} ({note}) → {location}; now `docsys fetch --root {}`",
        root.display()
    ))
}

/// `docsys consume discover <dir>`: every docsys tree one level under `dir`.
pub fn discover(root: &Path, dir: &Path) -> Result<Vec<Candidate>, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let have = consumed(&tree);
    let own = root.canonicalize().ok();
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("`{}`: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && !p
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    dirs.sort();
    let mut out = Vec::new();
    for d in dirs {
        let Some((provider_root, _)) = docs_root_of(&d) else {
            continue;
        };
        if own
            .as_ref()
            .is_some_and(|o| provider_root.canonicalize().ok().as_ref() == Some(o))
        {
            continue; // this tree itself
        }
        let Ok((ns, profile)) = namespace_of(&provider_root, &d) else {
            continue;
        };
        let already = have.contains(&ns);
        out.push(Candidate {
            ns,
            path: d,
            profile,
            already,
        });
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn names_become_local_ids() {
        assert_eq!(local_id_of("My_Project 2"), "my-project-2");
        assert_eq!(local_id_of("relay"), "relay");
        assert_eq!(local_id_of("--x--"), "x");
    }

    #[test]
    fn an_entry_lands_in_any_list_shape() {
        assert_eq!(
            with_entry("spec: docsys/0.4\n", "a=/p#docs"),
            "spec: docsys/0.4\nconsume: [a=/p#docs]\n"
        );
        assert_eq!(with_entry("consume: []\n", "a"), "consume: [a]\n");
        assert_eq!(
            with_entry("consume: [a, b]\nx: y\n", "c"),
            "consume: [a, b, c]\nx: y\n"
        );
        assert_eq!(
            with_entry("consume:\n  - a\n  - b\nx: y\n", "c"),
            "consume:\n  - a\n  - b\n  - c\nx: y\n"
        );
    }
}
