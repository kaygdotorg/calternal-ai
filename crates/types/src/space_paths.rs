//! Host-owned paths are independent from plugin kinds and item titles.

/// Standing instructions owned by the space's Managers and Owner. Generic kind
/// placement/writes cannot claim this file or use it as a directory. The host
/// reads exactly this path; there is no alias for an earlier filename.
pub const SPACE_INSTRUCTIONS_PATH: &str = "AI/AGENTS.md";

/// Whether an already validated, slash-separated relative path is the reserved
/// instructions file or a descendant of that file. Comparison ignores ASCII
/// case, matching the inherited portable-name key for this ASCII path.
/// Cost: at most two component comparisons; no allocation, I/O or lock.
pub fn is_reserved_space_path(path: &str) -> bool {
    let mut candidate = path.split('/');
    SPACE_INSTRUCTIONS_PATH.split('/').all(|part| {
        candidate
            .next()
            .is_some_and(|value| value.eq_ignore_ascii_case(part))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn instruction_file_and_directory_aliases_are_reserved() {
        for path in ["AI/AGENTS.md", "ai/agents.md", "AI/Agents.md/child"] {
            assert!(is_reserved_space_path(path), "{path}");
        }
        for path in [
            "AI",
            "AI/Chats/AGENTS.md",
            "Notes/AGENTS.md",
            "AI/AGENTS.md.backup",
        ] {
            assert!(!is_reserved_space_path(path), "{path}");
        }
    }
}
