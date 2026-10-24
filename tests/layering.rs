//! Layering rules of `docs/specs/01-architecture.md` §2 (VOL-ARCH-001, VOL-QA-007),
//! checked by scanning the source tree.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// All `.rs` files under `dir`, recursively (empty if `dir` does not exist).
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !dir.exists() {
        return out;
    }
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// Code part of a line: everything before a `//` comment.
fn code_of(line: &str) -> &str {
    line.split("//").next().unwrap_or("")
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Byte offsets where `word` occurs in `code` as a whole identifier.
fn word_ends<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    code.match_indices(word).filter_map(move |(i, _)| {
        let end = i + word.len();
        let before = code[..i].chars().next_back();
        let after = code[end..].chars().next();
        (!before.is_some_and(is_ident_char) && !after.is_some_and(is_ident_char)).then_some(end)
    })
}

/// True if `code` uses crate `name`: a `name::` path, or a `use`/`extern crate`
/// line naming it (catches `use name as alias;`). String literals such as
/// `"name-0"` do not count.
fn uses_crate(code: &str, name: &str) -> bool {
    let trimmed = code.trim_start().trim_start_matches("pub ");
    let is_import = trimmed.starts_with("use ") || trimmed.starts_with("extern crate ");
    word_ends(code, name).any(|end| is_import || code[end..].starts_with("::"))
}

/// `file:line: text` for every code line of `files` that uses one of `crates`.
fn offenders(files: &[PathBuf], crates: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for file in files {
        let text = fs::read_to_string(file).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = code_of(line);
            if crates.iter().any(|c| uses_crate(code, c)) {
                found.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
            }
        }
    }
    found
}

#[test]
fn model_and_coalesce_have_no_forbidden_imports() {
    let src = src_dir();
    let mut files = rust_files(&src.join("model"));
    assert!(!files.is_empty(), "src/model/ has no Rust files");
    let coalesce = src.join("backend").join("coalesce.rs");
    if coalesce.exists() {
        files.push(coalesce);
    }
    let found = offenders(
        &files,
        &["pipewire", "libspa", "ratatui", "crossterm", "tokio"],
    );
    assert!(found.is_empty(), "forbidden imports:\n{}", found.join("\n"));
}

#[test]
fn pipewire_only_in_backend_pw() {
    let src = src_dir();
    let pw = src.join("backend").join("pw");
    let files: Vec<PathBuf> = rust_files(&src)
        .into_iter()
        .filter(|f| !f.starts_with(&pw))
        .collect();
    let found = offenders(&files, &["pipewire", "libspa"]);
    assert!(
        found.is_empty(),
        "PipeWire used outside src/backend/pw/:\n{}",
        found.join("\n")
    );
}
