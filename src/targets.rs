//! Command-line file targeting: a directory or a glob pattern (`*`, `?`, `**`) stands for the
//! files it matches. Patterns are expanded here, once, since Windows shells pass them through.

use std::path::{Path, PathBuf};

/// `None` when `arg` is a plain path, to be used as given. Otherwise the matching files, sorted
/// (empty when nothing matches): a directory gives every `.dmn` file below it. Paths keep the
/// pattern's own spelling and separator.
pub fn expand(arg: &str) -> Option<Vec<String>> {
    let sep = if arg.contains('\\') { '\\' } else { '/' };
    let mut out = Vec::new();
    if !arg.contains(['*', '?']) {
        if !Path::new(arg).is_dir() {
            return None;
        }
        let shown = arg.trim_end_matches(['/', '\\']);
        let shown = if shown.is_empty() { &arg[..1] } else { shown };
        walk(Path::new(arg), shown, sep, &["**", "*.dmn"], &mut out);
    } else {
        let segs: Vec<&str> = arg.split(['/', '\\']).collect();
        let first = segs.iter().position(|s| s.contains(['*', '?'])).unwrap_or(0);
        let base = segs[..first].join(&sep.to_string());
        let dir = match (first, base.as_str()) {
            (0, _) => PathBuf::from("."),
            (_, "") => PathBuf::from(sep.to_string()),
            _ => PathBuf::from(&base),
        };
        let shown = if first > 0 && base.is_empty() { sep.to_string() } else { base };
        walk(&dir, &shown, sep, &segs[first..], &mut out);
    }
    out.sort();
    out.dedup();
    Some(out)
}

fn walk(dir: &Path, shown: &str, sep: char, segs: &[&str], out: &mut Vec<String>) {
    let Some((&seg, rest)) = segs.split_first() else { return };
    if seg == "**" {
        walk(dir, shown, sep, rest, out);
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let (Ok(ft), name) = (e.file_type(), e.file_name()) else { continue };
        let Some(name) = name.to_str() else { continue };
        // Like most shells, wildcards skip hidden entries unless the pattern names the dot.
        if name.starts_with('.') && !seg.starts_with('.') {
            continue;
        }
        let child = match shown {
            "" => name.to_string(),
            s if s.ends_with(sep) => format!("{s}{name}"),
            s => format!("{s}{sep}{name}"),
        };
        if seg == "**" {
            if ft.is_dir() {
                walk(&e.path(), &child, sep, segs, out);
            } else if rest.is_empty() {
                out.push(child);
            }
        } else if matches(seg, name) {
            if rest.is_empty() {
                if ft.is_file() {
                    out.push(child);
                }
            } else if ft.is_dir() {
                walk(&e.path(), &child, sep, rest, out);
            }
        }
    }
}

/// `*` matches any run of characters, `?` any one; case-insensitive on Windows, like its file names.
pub fn matches(pattern: &str, name: &str) -> bool {
    let fold = |c: char| if cfg!(windows) { c.to_ascii_lowercase() } else { c };
    let p: Vec<char> = pattern.chars().map(fold).collect();
    let n: Vec<char> = name.chars().map(fold).collect();
    let (mut i, mut j, mut star, mut mark) = (0, 0, None, 0);
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    p[i..].iter().all(|&c| c == '*')
}
