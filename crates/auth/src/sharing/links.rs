//! Recoverable item credentials and lookup by a keyed digest.

use super::*;
use crate::Auth;
use crate::db::now_ms;

/// Mutation to the public link owned by one item.
pub enum LinkEdit {
    /// Create an independently revocable credential with a display label.
    Create {
        /// Display name in the share sheet.
        label: String,
        /// Unix milliseconds, or no expiry.
        expires_at: Option<i64>,
    },
    /// Change an existing link without rotating its credential.
    Update {
        /// Link grant identifier.
        grant: Uuid,
        /// Replacement label, if supplied.
        label: Option<String>,
        /// Absent leaves the expiry unchanged. An inner None clears it.
        expires_at: Option<Option<i64>>,
    },
}

/// Bearer authorization projected from one live link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedLink {
    /// Link identifier used for rate limits and delivery checks.
    pub grant: Uuid,
    /// Stable item identity independent of its file name.
    pub item: Uuid,
    /// Registered content kind.
    pub kind: String,
}

fn validate(label: Option<&str>, expiry: Option<i64>, now: i64) -> Result<()> {
    if let Some(label) = label {
        calternal_core::validate_title(label).map_err(Error::invalid)?;
    }
    if expiry.is_some_and(|expiry| expiry <= now) {
        return Err(Error::invalid("link expiry must be in the future"));
    }
    Ok(())
}

impl Auth {
    /// Create or update a public link, checking current link management
    /// authority and committing the receipt with the encrypted secret.
    pub fn change_item_link(
        &self,
        actor: AccountId,
        space: SpaceId,
        id: Uuid,
        kind: String,
        edit: LinkEdit,
        receipt: (&str, &str, &str),
    ) -> Result<Value> {
        let key = self.inner.invite_key.item_links();
        self.spaces().mutate(actor, Some(space), Some(receipt), move |c| {
            require_editor(c, actor, space)?;
            if !link_admin(c, actor, space)? {
                return Err(Error::new(ErrorCode::Forbidden, "cannot manage links in this space"));
            }
            Ok(move |c: &rusqlite::Transaction<'_>| {
                let item = id.to_string();
                let space = space.to_string();
                let now = now_ms();
                let grant = match edit {
                    LinkEdit::Create { label, expires_at } => {
                        validate(Some(&label), expires_at, now)?;
                        c.execute(
                            "DELETE FROM item_grants WHERE item_id=?1 AND expires_at<=?2",
                            params![item, now],
                        ).map_err(sql)?;
                        let count: u32 = c.query_row(
                            "SELECT count(*) FROM item_grants WHERE item_id=?1 AND link_hash IS NOT NULL",
                            [&item],
                            |r| r.get(0),
                        ).map_err(sql)?;
                        if count >= LINK_LIMIT {
                            return Err(Error::invalid("this item already has 32 live links"));
                        }
                        let grant = Uuid::now_v7().to_string();
                        let token = crate::token::new_item_link_token()?;
                        let hash = key.lookup(&token);
                        let secret = key.encrypt(&token, &grant)?;
                        c.execute(
                            "INSERT INTO item_grants(id,item_id,kind,space_id,created_by,permission,link_hash,link_secret,label,expires_at)
                             VALUES(?1,?2,?3,?4,?5,1,?6,?7,?8,?9)",
                            params![grant, item, kind, space, actor.to_string(), hash.as_bytes(), secret, label, expires_at],
                        ).map_err(sql)?;
                        grant
                    }
                    LinkEdit::Update { grant, label, expires_at } => {
                        validate(label.as_deref(), expires_at.flatten(), now)?;
                        let grant = grant.to_string();
                        let changed = c.execute(
                            "UPDATE item_grants SET label=coalesce(?5,label),
                             expires_at=CASE WHEN ?6 THEN ?7 ELSE expires_at END
                             WHERE id=?1 AND item_id=?2 AND space_id=?3 AND link_hash IS NOT NULL
                             AND (expires_at IS NULL OR expires_at>?4)",
                            params![grant, item, space, now, label, expires_at.is_some(), expires_at.flatten()],
                        ).map_err(sql)?;
                        if changed == 0 {
                            return Err(missing());
                        }
                        grant
                    }
                };
                grant_row(c, &grant)
            })
        })
    }

    /// Recover a live link URL after checking current management authority.
    /// Performs one bounded auth worker read and never records the URL in a receipt.
    pub async fn copy_item_link(
        &self,
        actor: AccountId,
        space: SpaceId,
        item: Uuid,
        grant: Uuid,
    ) -> Result<String> {
        let key = self.inner.invite_key.item_links();
        let origin = self.inner.config.origin_header();
        self.inner
            .db
            .read(move |c| {
                require_editor(c, actor, space)?;
                if !link_admin(c, actor, space)? {
                    return Err(Error::new(
                        ErrorCode::Forbidden,
                        "cannot manage links in this space",
                    ));
                }
                let grant = grant.to_string();
                let blob: Vec<u8> = c.query_row(
                "SELECT link_secret FROM item_grants WHERE id=?1 AND item_id=?2 AND space_id=?3
                 AND link_hash IS NOT NULL AND (expires_at IS NULL OR expires_at>?4)",
                params![grant, item.to_string(), space.to_string(), now_ms()], |r| r.get(0),
            ).optional().map_err(sql)?.ok_or_else(missing)?;
                Ok(format!("{origin}/s/{}", key.decrypt(&blob, &grant)?))
            })
            .await
    }

    /// Resolve one canonical bearer to the current grant, without loading content
    /// or decrypting the secret. Invalid, expired and revoked tokens return None.
    pub async fn resolve_item_link(&self, token: &str) -> Result<Option<ResolvedLink>> {
        if !crate::token::plausible_item_link_token(token) {
            return Ok(None);
        }
        let digest = self.inner.invite_key.item_links().lookup(token);
        self.inner.db.read(move |c| {
            let tuple: Option<(String, String, String)> = c.query_row(
                "SELECT id,item_id,kind FROM item_grants WHERE link_hash=?1 AND (expires_at IS NULL OR expires_at>?2)",
                params![digest.as_bytes(), now_ms()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).optional().map_err(sql)?;
            tuple.map(|(grant, item, kind)| Ok(ResolvedLink {
                grant: Uuid::parse_str(&grant).map_err(Error::internal)?,
                item: Uuid::parse_str(&item).map_err(Error::internal)?,
                kind,
            })).transpose()
        }).await
    }

    /// Delete up to 256 expired link credentials through protected publication.
    /// Returns true when another batch may be ready. Empty polls only read SQL.
    pub async fn purge_expired_item_links(&self) -> Result<bool> {
        self.inner
            .db
            .run(|c| {
                let now = now_ms();
                let pending: bool = c
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM item_grants WHERE expires_at<=?1)",
                        [now],
                        |r| r.get(0),
                    )
                    .map_err(sql)?;
                if !pending {
                    return Ok(false);
                }
                let tx = c
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(sql)?;
                let count = tx
                    .execute(
                        "DELETE FROM item_grants WHERE id IN (
                    SELECT id FROM item_grants WHERE expires_at<=?1 ORDER BY expires_at LIMIT 256
                 )",
                        [now],
                    )
                    .map_err(sql)?;
                crate::spaces::commit(tx)?;
                Ok(count == 256)
            })
            .await
    }

    /// Recheck a previously resolved guest grant before delivery. Uses its
    /// primary key and current server time in one auth-worker read.
    pub async fn item_link_is_live(&self, grant: Uuid) -> Result<bool> {
        self.inner
            .db
            .read(move |c| {
                c.query_row(
                    "SELECT EXISTS(SELECT 1 FROM item_grants WHERE id=?1 AND link_hash IS NOT NULL
             AND (expires_at IS NULL OR expires_at>?2))",
                    params![grant.to_string(), now_ms()],
                    |r| r.get(0),
                )
                .map_err(sql)
            })
            .await
    }
}
