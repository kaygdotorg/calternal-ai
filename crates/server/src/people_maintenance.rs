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
    }
}
