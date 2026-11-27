//! Ensure production routing derives app and provider identities from bundles.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn generic_sources_contain_no_chat_or_claude_code_identities() {
    let host = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let machine = host.parent().unwrap().join("machine");
    let result = Command::new("grep")
        .args([
            "-rEi",
            "--include=*.rs",
            "--exclude=tests.rs",
            "--exclude=*_tests.rs",
            "--",
            r"\b(chat|chats|calternal-(chat|reading-list|tasks)|claude|anthropic)\b|CLAUDE_(CODE|LOGIN)|claudeAiOauth",
        ])
        .arg(host.join("src"))
        .arg(machine.join("src"))
        .output()
        .expect("dev environment must supply grep");
    assert_eq!(
        result.status.code(),
        Some(1),
        "generic Rust sources refer to an app, kind or provider:\n{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
    );
}
