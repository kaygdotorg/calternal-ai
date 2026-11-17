//! Explicit preview fixture provisioning.
use super::*;

/// Persistent development fixture identities; only explicit preview startup uses this.
#[derive(Debug, Clone)]
pub struct DevelopmentIds {
    /// Account associated with the development owner token.
    pub owner: AccountId,
    /// Account associated with the development guest token.
    pub guest: AccountId,
    /// The development owner's provisioned personal space UUID.
    pub personal: SpaceId,
    /// The space named `shared`, where the guest is a reader.
    pub shared: SpaceId,
}
impl Spaces {
    /// Creates development fixtures exactly once, preserving later revocations,
    /// transfers and deletions across restart. Never call for real installations.
    /// `development_directory` records what it created so a restart returns the same
    /// IDs; nothing on the request path reads it, and requests never use the names.
    /// Accounts and spaces each receive a separate freshly allocated UUIDv7.
    /// A configured recovery barrier also covers this explicit fixture mutation.
    pub fn seed_development(&self) -> Result<DevelopmentIds> {
        self.run(move |c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            let existing: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM development_directory)",
                    [],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            if !existing {
                let owner = AccountId::new();
                let guest = AccountId::new();
                for (name, id) in [("owner", owner), ("guest", guest)] {
                    let personal = crate::setup::insert_account(
                        &tx,
                        id,
                        name,
                        &format!("dev-{name}"),
                        crate::db::now_ms(),
                    )?;
                    if id == owner {
                        tx.execute(
                            "INSERT INTO development_directory VALUES('personal',?1)",
                            [personal.to_string()],
                        )
                        .map_err(sql)?;
                    }
                    tx.execute(
                        "INSERT INTO development_directory VALUES(?1,?2)",
                        params![name, id.to_string()],
                    )
                    .map_err(sql)?;
                }
                {
                    let s = create(&tx, owner, "shared")?;
                    tx.execute(
                        "INSERT INTO development_directory VALUES(?1,?2)",
                        params!["shared", s.id.to_string()],
                    )
                    .map_err(sql)?;
                    tx.execute(
                        "INSERT INTO space_memberships VALUES(?1,?2,'reader')",
                        params![s.id.to_string(), guest.to_string()],
                    )
                    .map_err(sql)?;
                }
            }
            let get = |name| {
                tx.query_row(
                    "SELECT id FROM development_directory WHERE name=?1",
                    [name],
                    |r| r.get::<_, String>(0),
                )
                .map_err(sql)
            };
            let ids = DevelopmentIds {
                owner: account(get("owner")?)?,
                guest: account(get("guest")?)?,
                personal: space_id(get("personal")?)?,
                shared: space_id(get("shared")?)?,
            };
            commit(tx)?;
            Ok(ids)
        })
    }
}
