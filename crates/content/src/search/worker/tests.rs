use super::*;
use rusqlite::ErrorCode;

#[test]
fn large_rebuild_cannot_grow_a_wal_pinned_by_a_reader() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("search-v2.db");
    let mut writer = open_writer(&path).unwrap();
    let reader = crate::pool::open_reader(&path).unwrap();
    reader
        .execute_batch("BEGIN; SELECT count(*) FROM search_docs;")
        .unwrap();

    let mut committed = 0;
    let mut reached_limit = false;
    for _ in 0..512 {
        let batch = writer.transaction().unwrap();
        let write = batch.execute(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<128)
             INSERT INTO search_text(title,body) SELECT i+?1,hex(randomblob(2048)) FROM n",
            [committed],
        );
        let result = write.and_then(|_| batch.commit());
        assert!(
            std::fs::metadata(path.with_extension("db-wal"))
                .unwrap()
                .len()
                <= 64 * 1024 * 1024
        );
        match result {
            Ok(()) => committed += 128,
            Err(error) => {
                assert_eq!(error.sqlite_error_code(), Some(ErrorCode::DiskFull));
                reached_limit = true;
                break;
            }
        }
    }
    assert!(reached_limit && committed > 1024);
    assert!(!checkpoint_if_needed(&writer, &path).unwrap());
    reader.execute_batch("ROLLBACK").unwrap();
    assert!(checkpoint_if_needed(&writer, &path).unwrap());
    assert_eq!(
        std::fs::metadata(path.with_extension("db-wal"))
            .unwrap()
            .len(),
        0
    );
    let seen: i64 = writer
        .query_row("SELECT count(*) FROM search_text", [], |row| row.get(0))
        .unwrap();
    assert_eq!(seen, committed);
    let state = reset(
        &mut writer,
        &SearchConfig::new(Vec::new()),
        SpaceId::new(),
        &SyncToken {
            epoch: "test".into(),
            seq: 0,
        },
    )
    .unwrap();
    assert!(!state.ready);
    let remaining: i64 = writer
        .query_row("SELECT count(*) FROM search_text", [], |row| row.get(0))
        .unwrap();
    assert_eq!(remaining, 0);
    writer
        .execute(
            "INSERT INTO search_text(title,body) VALUES('ready','resumed')",
            [],
        )
        .unwrap();
}
