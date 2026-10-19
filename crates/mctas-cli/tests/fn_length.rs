//! No function of the CLI crates (`crates/door-*`, `crates/mctas*`) may grow past
//! `MAX_BODY_LINES` lines: a command is a `cmd_<name>` function in its family's module and its
//! work goes in piston-door ([docs/internals.md#build](../../../docs/internals.md#build)), so no
//! dispatcher becomes one gigantic function again.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The longest function body allowed, in lines (between the braces' lines).
const MAX_BODY_LINES: usize = 150;

/// Functions that were already longer when this test came: (file, function, its length then).
/// They may only shrink; remove an entry once its function is within `MAX_BODY_LINES`.
const TODO: &[(&str, &str, usize)] = &[];

#[derive(Debug, PartialEq)]
enum Tok {
    Ident(String),
    Punct(char),
}

/// Rust tokens with their line (1-based), comments, strings and char literals skipped.
fn lex(text: &str) -> Vec<(Tok, usize)> {
    let c: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    let ident_start = |ch: char| ch.is_alphabetic() || ch == '_';
    let ident_char = |ch: char| ch.is_alphanumeric() || ch == '_';
    while i < c.len() {
        let ch = c[i];
        if ch == '\n' {
            line += 1;
            i += 1;
        } else if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if c[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
            }
        } else if ch == '"' {
            i += 1;
            while i < c.len() && c[i] != '"' {
                if c[i] == '\\' {
                    i += 1;
                }
                if c.get(i) == Some(&'\n') {
                    line += 1;
                }
                i += 1;
            }
            i += 1;
        } else if ch == '\'' {
            if c.get(i + 1) == Some(&'\\') {
                // an escaped char literal: '\n', '\'', '\u{..}'
                i += 2;
                while i < c.len() && c[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if c.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                // a lifetime or a label
                i += 1;
                while i < c.len() && ident_char(c[i]) {
                    i += 1;
                }
            }
        } else if ident_start(ch) {
            let s = i;
            while i < c.len() && ident_char(c[i]) {
                i += 1;
            }
            let word: String = c[s..i].iter().collect();
            // raw strings: r"..", r#".."#, br#".."#
            if matches!(word.as_str(), "r" | "br" | "cr") {
                let mut j = i;
                while c.get(j) == Some(&'#') {
                    j += 1;
                }
                if c.get(j) == Some(&'"') {
                    let hashes = j - i;
                    i = j + 1;
                    loop {
                        if i >= c.len() {
                            break;
                        }
                        if c[i] == '"' && (0..hashes).all(|k| c.get(i + 1 + k) == Some(&'#')) {
                            i += 1 + hashes;
                            break;
                        }
                        if c[i] == '\n' {
                            line += 1;
                        }
                        i += 1;
                    }
                    continue;
                }
            }
            out.push((Tok::Ident(word), line));
        } else if ch.is_ascii_digit() {
            while i < c.len() && ident_char(c[i]) {
                i += 1;
            }
        } else {
            out.push((Tok::Punct(ch), line));
            i += 1;
        }
    }
    out
}

/// Every function with a body: (name, line of `fn`, body lines between the braces' lines).
fn functions(text: &str) -> Vec<(String, usize, usize)> {
    let toks = lex(text);
    // the opening brace of each function body
    let mut opens: BTreeMap<usize, (String, usize)> = BTreeMap::new();
    for i in 0..toks.len() {
        if toks[i].0 != Tok::Ident("fn".into()) {
            continue;
        }
        let Some((Tok::Ident(name), _)) = toks.get(i + 1) else { continue };
        let mut depth = 0i32;
        for (j, (t, _)) in toks.iter().enumerate().skip(i + 2) {
            match t {
                Tok::Punct('(' | '[') => depth += 1,
                Tok::Punct(')' | ']') => depth -= 1,
                Tok::Punct(';') if depth == 0 => break,
                Tok::Punct('{') if depth == 0 => {
                    opens.insert(j, (name.clone(), toks[i].1));
                    break;
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    let mut stack: Vec<(String, usize, usize, i32)> = Vec::new();
    let mut depth = 0i32;
    for (j, (t, line)) in toks.iter().enumerate() {
        match t {
            Tok::Punct('{') => {
                depth += 1;
                if let Some((name, fn_line)) = opens.get(&j) {
                    stack.push((name.clone(), *fn_line, *line, depth));
                }
            }
            Tok::Punct('}') => {
                if stack.last().is_some_and(|s| s.3 == depth) {
                    let (name, fn_line, open_line, _) = stack.pop().unwrap();
                    out.push((name, fn_line, line.saturating_sub(open_line + 1)));
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    out
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn lexer_skips_strings_comments_and_chars() {
    let f = functions("fn a() {\n let s = \"}\";\n let c = '}';\n // }\n /* } */ let r = r#\"}\"#;\n}\nfn b<'x>(v: &'x str) -> impl Fn() -> u8 { || 1 }\ntrait T { fn c(&self); }\n");
    assert_eq!(f, vec![("a".to_string(), 1, 4), ("b".to_string(), 7, 0)]);
}

#[test]
fn cli_functions_stay_short() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    let mut crates: Vec<PathBuf> = std::fs::read_dir(root.join("crates")).unwrap().filter_map(|e| e.ok().map(|e| e.path())).collect();
    crates.sort();
    for c in crates {
        let name = c.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with("door-") || name.starts_with("mctas") {
            rust_files(&c.join("src"), &mut files);
        }
    }
    assert!(files.len() > 20, "found only {} files under {}", files.len(), root.display());
    let mut errors = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(f).unwrap();
        for (name, line, n) in functions(&text) {
            let allowed = TODO.iter().find(|(file, func, _)| *file == rel && *func == name).map(|t| t.2).unwrap_or(MAX_BODY_LINES);
            if n > allowed {
                errors.push(format!("{rel}:{line} fn {name}: {n} lines (at most {allowed})"));
            }
        }
    }
    assert!(
        errors.is_empty(),
        "functions too long: split each into smaller functions (a command is one `cmd_<name>` in its family's module, its work goes in piston-door: docs/internals.md#build)\n{}",
        errors.join("\n")
    );
}
