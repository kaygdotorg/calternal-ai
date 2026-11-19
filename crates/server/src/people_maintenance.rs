//! Server-lifetime maintenance for bounded authority rows.

pub(crate) async fn sweep_receipts(spaces: calternal_auth::spaces::Spaces) {
    let hour = std::time::Duration::from_secs(3600);
    let mut schedule = tokio::time::interval_at(tokio::time::Instant::now() + hour, hour);
    schedule.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        schedule.tick().await;
        if let Err(error) = spaces.maintain_receipts().await {
            tracing::warn!(%error, "could not expire directory receipts or retire their history");
        }
    }
}

pub(crate) async fn sweep_invites(auth: calternal_auth::Auth, mut backlog: bool) {
    loop {
        if backlog {
            tokio::task::yield_now().await;
        } else {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
        backlog = match auth.purge_invites().await {
            Ok(backlog) => backlog,
            Err(error) => {
                tracing::warn!(%error, "could not purge terminal invitations");
                false
            }
        };
        match auth.purge_expired_item_links().await {
            Ok(link_backlog) => backlog |= link_backlog,
            Err(error) => tracing::warn!(%error, "could not purge expired item links"),
        }
    }
}

pub(crate) async fn sweep_purged_sharing(
    authority: calternal_auth::spaces::Spaces,
    content: std::sync::Arc<calternal_content::Content>,
) {
    let mut after = None;
    loop {
        let result = authority.sharing_cleanup_page(after).await;
        match result {
            Ok(page) => {
                after = if page.len() == 64 {
                    page.last().copied()
                } else {
                    None
                };
                for (space, id) in page {
                    let content = content.clone();
                    let state =
                        tokio::task::spawn_blocking(move || content.item_state(space, id)).await;
                    if matches!(state, Ok(Ok("deleted")))
                        && let Err(error) = authority.purge_item_grants(space, id).await
                    {
                        tracing::warn!(%space, %id, %error, "could not remove shares for a purged item");
                    }
                }
            }
            Err(error) => tracing::warn!(%error, "could not enumerate sharing cleanup"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

pub(crate) async fn sweep_account_security(auth: calternal_auth::Auth) {
    loop {
        match auth.purge_account_security().await {
            Ok(true) => tokio::task::yield_now().await,
            outcome => {
                if outcome.is_err() {
                    tracing::warn!("could not prune expired account security records");
                }
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        }
    }
}
