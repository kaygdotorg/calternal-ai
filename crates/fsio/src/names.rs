//! File names people can trust.
//!
//! Names are stored in Unicode NFC, the form most tools use. Two names
//! that differ only by case or by Unicode form (macOS and some DAV clients
//! send NFD) count as the same name, so they can never become two notes
//! that look identical.

use unicode_normalization::UnicodeNormalization;

/// The name in NFC form.
pub fn nfc(name: &str) -> String {
    // Plain ASCII is already NFC (most names; much cheaper at 100,000).
    if name.is_ascii() {
        return name.to_owned();
    }
    name.nfc().collect()
}

/// The comparison key: NFC, then lowercased. Two names with the same key
/// are the same name.
pub fn fold(name: &str) -> String {
    if name.is_ascii() {
        return name.to_ascii_lowercase();
    }
    nfc(name).to_lowercase().nfc().collect()
}

/// Turns a title into a safe file stem: NFC, no path separators or
/// control characters, no leading dots, trimmed, and at most 120
/// characters. Empty titles become `Untitled`.
pub fn stem_from_title(title: &str) -> String {
    let cleaned: String = nfc(title)
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let mut stem = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_start_matches('.')
        .trim()
        .to_owned();
    if stem.chars().count() > 120 {
        stem = stem.chars().take(120).collect::<String>().trim().to_owned();
    }
    if stem.is_empty() {
        "Untitled".to_owned()
    } else {
        stem
    }
}

/// Picks `<stem>.md`, or `<stem> 2.md`, `<stem> 3.md`… so the name doesn't
/// clash with any existing name (ignoring case and Unicode form).
pub fn unique_md_name(stem: &str, taken: impl Fn(&str) -> bool) -> String {
    let first = format!("{stem}.md");
    if !taken(&fold(&first)) {
        return first;
    }
    (2..)
        .map(|n| format!("{stem} {n}.md"))
        .find(|name| !taken(&fold(name)))
        .expect("an unused name always exists")
}

mod directory;
pub use directory::DirectoryNames;
