//! Recheck the surviving authority before evicting any active rooms.

use super::*;

pub(super) async fn after_revoke<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    kind: &str,
    item: Uuid,
    audience: Audience,
) -> Result<(), Error> {
    async fn check<D: Directory>(
        state: &Shared<D>,
        space: SpaceId,
        kind: &str,
        item: Uuid,
        account: AccountId,
    ) -> Result<(), Error> {
        let (caps, _) =
            crate::resolve::item_capabilities(state, account, space, kind, item).await?;
        if !caps.is_some_and(|c| c.contains(calternal_types::Capability::SpaceRead)) {
            state
                .directory
                .room_eviction()
                .evict_item_account(space, kind, item, account)
                .await;
        }
        Ok(())
    }
    match audience {
        Audience::Account(account) => check(state, space, kind, item, account).await,
        Audience::Group(group) => {
            let authority = state
                .directory
                .protected_spaces()
                .ok_or_else(|| Error::unavailable("stored space directory unavailable"))?;
            let mut after = None;
            loop {
                let accounts = authority
                    .eviction_group_accounts(group, after)
                    .await
                    .map_err(auth_error)?;
                let last = accounts.last().copied();
                for account in accounts {
                    check(state, space, kind, item, account).await?;
                }
                let Some(last) = last else {
                    return Ok(());
                };
                after = Some(last);
            }
        }
    }
}
