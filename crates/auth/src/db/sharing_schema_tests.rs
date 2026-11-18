//! The dormant grant schema upgrades the accounts release and follows erasure.

use super::*;
use calternal_store::migrations::{Schema, inspect, migrate};

const ACCOUNTS_RELEASE: Schema<'_> = Schema::new(
    "auth.db",
    11,
    &[include_str!("../schema.sql")],
    &[
        include_str!("../account_details.sql"),
        include_str!("../oidc.sql"),
    ],
);

#[test]
fn auth_13_to_14_preserves_accounts_and_covers_sharing_erasure() {
    let sharing_release = Schema::new(
        "auth.db",
        11,
        &[include_str!("../schema.sql")],
        &AUTH_CHANGES[..3],
    );
    let connection = Connection::open_in_memory().unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    migrate(&connection, ACCOUNTS_RELEASE).unwrap();
    assert_eq!(inspect(&connection, ACCOUNTS_RELEASE).unwrap(), 13);
    connection
        .execute_batch(
            "BEGIN;
         INSERT INTO users(id,name,username,created_at) VALUES
           ('owner','Owner','owner',100),('viewer','Viewer','viewer',101),
           ('creator','Creator','creator',102);
         INSERT INTO spaces(id,name,owner_account,created) VALUES('space','Test space','owner',100);
         INSERT INTO space_memberships VALUES('space','owner','manager');
         INSERT INTO groups(id,name) VALUES('group','Test group');
         COMMIT;",
        )
        .unwrap();

    connection
        .execute("CREATE TABLE item_grants(collision TEXT)", [])
        .unwrap();
    assert!(migrate(&connection, sharing_release).is_err());
    assert_eq!(inspect(&connection, ACCOUNTS_RELEASE).unwrap(), 13);
    connection.execute("DROP TABLE item_grants", []).unwrap();
    migrate(&connection, sharing_release).unwrap();
    assert_eq!(inspect(&connection, sharing_release).unwrap(), 14);
    let username: String = connection
        .query_row("SELECT username FROM users WHERE id='owner'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(username, "owner");

    connection.execute_batch(
        "INSERT INTO item_grants(id,item_id,kind,space_id,created_by,account_id,permission)
           VALUES('direct','item','notes','space','creator','viewer',2);
         INSERT INTO item_grants(id,item_id,kind,space_id,created_by,group_id,permission)
           VALUES('group-grant','item','notes','space','creator','group',1);
         INSERT INTO item_grants(id,item_id,kind,space_id,created_by,link_hash,link_secret,label,permission)
           VALUES('link','item','notes','space','creator',X'01',X'02','Public',1);
         INSERT INTO item_link_policies VALUES('space');
         INSERT INTO item_share_seen VALUES('viewer','space','item');
         DELETE FROM users WHERE id='viewer';
         DELETE FROM groups WHERE id='group';
         DELETE FROM users WHERE id='creator';",
    )
    .unwrap();
    let count = |table: &str| -> i64 {
        connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    assert_eq!(count("item_share_seen"), 0);
    assert_eq!(count("item_grants"), 1);
    let survivor: (String, Option<String>) = connection
        .query_row("SELECT id,created_by FROM item_grants", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(survivor, ("link".into(), None));
    connection
        .execute("DELETE FROM spaces WHERE id='space'", [])
        .unwrap();
    assert_eq!(count("item_grants"), 0);
    assert_eq!(count("item_link_policies"), 0);
}
