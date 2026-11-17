//! Manifest validation excludes the Manager-owned standing-instructions path.
use calternal_plugin_host::{parse_manifest, validate_manifest};

#[test]
fn manifest_kinds_cannot_claim_the_reserved_instruction_file_as_a_directory() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/calternal-orchestrator/manifest.json");
    let manifest = parse_manifest(&std::fs::read(root).unwrap()).unwrap();
    for parts in [vec!["AGENTS.md"], vec!["agents.md", "child"]] {
        let mut invalid = manifest.clone();
        invalid.kinds=vec![serde_json::from_value(serde_json::json!({"id":"reserved-test","storage":{"folder":"AI","subdirectories":parts},"claimed_text":true,"schema":true})).unwrap()];
        assert!(validate_manifest(&invalid).is_err());
    }
}
