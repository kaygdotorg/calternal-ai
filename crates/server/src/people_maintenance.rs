//! Server-lifetime maintenance for bounded authority rows.

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
