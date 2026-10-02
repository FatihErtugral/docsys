//! A tree pins the docsys it runs (D-120). The pin is one line, a version,
//! in `<root>/.docsys-version` — the implementation's own configuration,
//! which R-162 keeps out of `.docmeta.yml`. Every command that works on a
//! pinned tree runs that version: this binary when it is the one, otherwise
//! the pinned binary from the version cache, installed once on first use.
//! A tree without a pin (every tree 0.15 wrote) is served by this binary,
//! by its declared spec (D-118).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The pin's file, beside `.docmeta.yml`.
pub const FILE: &str = ".docsys-version";

/// Set on the pinned binary a dispatch runs: it never dispatches again. It is
/// that one process's: every process docsys starts runs without it.
pub const GUARD: &str = "DOCSYS_DISPATCHED";

/// Set by a person who installs every version by hand.
pub const NO_AUTO_INSTALL: &str = "DOCSYS_NO_AUTO_INSTALL";

/// This binary's version.
pub fn own() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// `x.y.z`, digits only.
pub fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.split('.').map(|p| {
        (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse::<u64>().ok())
            .flatten()
    });
    let out = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(out)
}

/// The tree's pin: `Ok(None)` when it has none, `Err` naming a pin that is
/// not a version.
pub fn read(root: &Path) -> Result<Option<String>, String> {
    let Ok(text) = std::fs::read_to_string(root.join(FILE)) else {
        return Ok(None);
    };
    let v = text.lines().next().unwrap_or("").trim().to_string();
    if parse(&v).is_none() {
        return Err(format!(
            "`{}` holds `{v}`, not a version (`x.y.z`) — `docsys upgrade` writes it",
            root.join(FILE).display()
        ));
    }
    Ok(Some(v))
}

/// Write the pin: this binary's version.
pub fn write(root: &Path) -> Result<(), String> {
    std::fs::write(root.join(FILE), format!("{}\n", own())).map_err(|e| e.to_string())
}

/// Where the pinned versions live: `$DOCSYS_HOME`, else `~/.docsys`.
pub fn home() -> Option<PathBuf> {
    if let Some(h) = std::env::var_os("DOCSYS_HOME").filter(|h| !h.is_empty()) {
        return Some(PathBuf::from(h));
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(|h| PathBuf::from(h).join(".docsys"))
}

fn binary(home: &Path, v: &str) -> PathBuf {
    let name = if cfg!(windows) {
        "docsys.exe"
    } else {
        "docsys"
    };
    home.join("versions").join(v).join("bin").join(name)
}

/// The one command that installs version `v` where dispatch looks for it.
pub fn install_command(home: &Path, v: &str) -> String {
    format!(
        "cargo install docsys --version {v} --locked --root {}",
        home.join("versions").join(v).display()
    )
}

/// Run the pinned version in place of this process. Returns only when this
/// binary is to run: no pin, the pin is this version, or the guard is set.
/// Otherwise it exits with the pinned binary's code, or with 1 and one line
/// when the version is not installed and may not be installed here — an
/// agent hook (`installs: false`) never waits for a compile.
pub fn run_pinned(root: &Path, installs: bool) -> Result<(), std::process::ExitCode> {
    if std::env::var_os(GUARD).is_some() {
        return Ok(());
    }
    let pin = match read(root) {
        Ok(Some(v)) if v != own() => v,
        Ok(_) => return Ok(()),
        Err(e) => {
            eprintln!("docsys: {e}");
            return Err(std::process::ExitCode::from(2));
        }
    };
    let Some(home) = home() else {
        eprintln!(
            "docsys: this tree pins docsys {pin}; set DOCSYS_HOME to where its versions live"
        );
        return Err(std::process::ExitCode::from(1));
    };
    let bin = binary(&home, &pin);
    if !bin.is_file() {
        let command = install_command(&home, &pin);
        let cargo = || {
            Command::new("cargo")
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        };
        let allowed = installs && std::env::var_os(NO_AUTO_INSTALL).is_none() && cargo();
        if !allowed {
            eprintln!("docsys: this tree pins docsys {pin}; install it: {command}");
            return Err(std::process::ExitCode::from(1));
        }
        eprintln!("docsys: this tree pins docsys {pin}; installing it once…");
        let installed = Command::new("cargo")
            .args(["install", "docsys", "--version", &pin, "--locked", "--root"])
            .arg(home.join("versions").join(&pin))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !installed || !bin.is_file() {
            eprintln!("docsys: docsys {pin} could not be installed; install it: {command}");
            return Err(std::process::ExitCode::from(1));
        }
    }
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let mut cmd = Command::new(&bin);
    cmd.args(&args).env(GUARD, &pin);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let e = cmd.exec();
        eprintln!("docsys: {} could not run: {e}", bin.display());
        Err(std::process::ExitCode::from(1))
    }
    #[cfg(not(unix))]
    {
        match cmd.status() {
            Ok(s) => Err(std::process::ExitCode::from(
                u8::try_from(s.code().unwrap_or(1)).unwrap_or(1),
            )),
            Err(e) => {
                eprintln!("docsys: {} could not run: {e}", bin.display());
                Err(std::process::ExitCode::from(1))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_three_numbers() {
        assert_eq!(parse("0.16.0"), Some((0, 16, 0)));
        assert_eq!(parse("10.2.33"), Some((10, 2, 33)));
        for bad in [
            "", "0.16", "0.16.0.1", "0.16.x", "v0.16.0", "0..1", " 0.16.0",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }
}
