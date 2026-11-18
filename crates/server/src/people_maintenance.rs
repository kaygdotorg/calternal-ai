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
