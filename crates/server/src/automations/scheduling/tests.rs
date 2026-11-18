use super::*;

fn fixture() -> (Connection, SpaceId, Uuid) {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn, SCHEMA).unwrap();
    (conn, SpaceId::new(), Uuid::now_v7())
}

#[test]
fn occurrence_key_survives_a_lost_handoff_response() {
    let (mut conn, space, automation) = fixture();
    let tx = conn.transaction().unwrap();
    let run = queue(
        &tx,
        space,
        automation,
        "revision".into(),
        Some(100),
        None,
        100,
    )
    .unwrap()
    .unwrap();
    tx.commit().unwrap();
    let recovered: String = conn
        .query_row("SELECT id FROM runs WHERE scheduled=100", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(recovered, run.id.to_string());
    let tx = conn.transaction().unwrap();
    assert!(
        queue(
            &tx,
            space,
            automation,
            "revision".into(),
            Some(100),
            None,
            101
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn a_space_has_at_most_thirty_two_open_handoffs() {
    let (mut conn, space, _) = fixture();
    let tx = conn.transaction().unwrap();
    for _ in 0..MAX_QUEUED {
        assert!(
            queue(
                &tx,
                space,
                Uuid::now_v7(),
                "revision".into(),
                None,
                None,
                100
            )
            .unwrap()
            .is_some()
        );
    }
    assert!(
        queue(
            &tx,
            space,
            Uuid::now_v7(),
            "revision".into(),
            None,
            None,
            100
        )
        .unwrap()
        .is_none()
    );
    assert!(
        queue(
            &tx,
            SpaceId::new(),
            Uuid::now_v7(),
            "revision".into(),
            None,
            None,
            100
        )
        .unwrap()
        .is_some()
    );
}
