//! The release inventory shares its descriptors with the writers.
use super::*;
use calternal_store::migrations::migrate;
use regex::Regex;

fn visit_sources(root: &Path, schema_call: &Regex, names: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_str().unwrap();
        if path.is_dir() && file != "tests" {
            visit_sources(&path, schema_call, names);
        } else if file.ends_with(".rs") && file != "tests.rs" && !file.ends_with("_tests.rs") {
            let source = std::fs::read_to_string(&path).unwrap();
            for call in schema_call.captures_iter(&source) {
                let name = &call[1];
                assert!(
                    name.starts_with('"') && name.ends_with('"'),
                    "{}: use a unique literal SQLite descriptor name",
                    path.display()
                );
                names.insert(name.trim_matches('"').to_owned());
            }
        }
    }
}

#[test]
fn every_sqlite_writer_declares_its_release_policy() {
    let mut covered = system_stores()
        .into_iter()
        .chain(space_stores())
        .chain(account_stores())
        .map(|(_, schema)| schema.name().to_owned())
        .collect::<BTreeSet<_>>();
    // Versioned search projections are rebuilt from durable sources and are
    // deliberately outside the authority migration/snapshot loop.
    covered.extend(["search projection", "message search projection"].map(str::to_owned));

    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let call = Regex::new(r#"\bSchema\s*::\s*new\s*\(\s*("[^"]+"|[^,\s]+)"#).unwrap();
    let mut writers = BTreeSet::new();
    for crate_dir in std::fs::read_dir(workspace).unwrap() {
        let sources = crate_dir.unwrap().path().join("src");
        if sources.is_dir() {
            visit_sources(&sources, &call, &mut writers);
        }
    }
    assert_eq!(
        writers, covered,
        "declare every SQLite writer in the release or rebuildable projection inventory"
    );
}

#[test]
fn every_authority_admits_only_its_exact_additive_successor() {
    for (file, current) in system_stores()
        .into_iter()
        .chain(space_stores())
        .chain(account_stores())
    {
        let db = Connection::open_in_memory().unwrap();
        let have = current.version();
        let next = have + 1;
        db.pragma_update(None, "user_version", have).unwrap();
        let steps = [(
            next,
            "CREATE TABLE additive_fixture(id INTEGER PRIMARY KEY) STRICT;",
        )];
        let future = Schema::new(current.name(), have, &[], &steps);
        migrate_release(&db, future, true).unwrap();
        assert_eq!(inspect(&db, current).unwrap(), next, "{file}");
        migrate(&db, current).unwrap();
        db.execute(
            "UPDATE _calternal_schema_compatibility SET older=older-1",
            [],
        )
        .unwrap();
        assert!(inspect(&db, current).is_err(), "{file}");
    }
}
