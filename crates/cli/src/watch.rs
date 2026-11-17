//! Shared watch adapter used by both command-line binaries.
use crate::output::{CliError, Output, Reply};
use calternal_client::{CancellationToken, Client, FileBookmarks, MemoryBookmarks};
use calternal_core::LiveFrame;
use std::{path::PathBuf, sync::Arc};

/// Follow one space, writing each event through the shared output formatter.
/// `bookmarks` must be exclusive to this server/account; absent means memory only.
/// Cost: one live socket with bounded buffering, one output write per event, and
/// optional atomic bookmark writes. Returns on reset, cancellation or refusal.
pub async fn run(
    base_url: &str,
    token_file: PathBuf,
    space: String,
    bookmarks: Option<PathBuf>,
    out: &Output,
    cancellation: CancellationToken,
) -> Result<Reply, CliError> {
    let token = tokio::task::spawn_blocking(move || {
        use std::io::Read;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&token_file)
                .map_err(|_| CliError::new("unauthorized", "Cannot read the credential file."))?;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(
                    CliError::new("unauthorized", "Credential file must be owner-only.")
                        .hint("Run chmod 600 on the credential file."),
                );
            }
        }
        let mut token = String::new();
        std::fs::File::open(token_file)
            .map_err(|_| CliError::new("unauthorized", "Cannot open the credential file."))?
            .take(4097)
            .read_to_string(&mut token)
            .map_err(|_| CliError::new("unauthorized", "Cannot read the credential file."))?;
        if token.len() > 4096 {
            return Err(CliError::new(
                "unauthorized",
                "Credential file is too large.",
            ));
        }
        Ok(token.trim().to_owned())
    })
    .await
    .map_err(|_| CliError::new("internal", "Credential task stopped."))??;
    let client = Client::new(base_url, &token).map_err(error)?;
    let storage: Arc<dyn calternal_client::BookmarkStorage> = match bookmarks {
        Some(path) => Arc::new(FileBookmarks::new(path)),
        None => Arc::new(MemoryBookmarks::default()),
    };
    let mut live = client
        .subscribe(vec![space], storage, cancellation)
        .await
        .map_err(error)?;
    loop {
        let frame = match live.next().await {
            Ok(frame) => frame,
            Err(calternal_client::ClientError::Cancelled) => {
                return Ok(Reply::new(serde_json::json!({"stopped":true})).line("Watch stopped."));
            }
            Err(e) => return Err(error(e)),
        };
        let value = serde_json::to_value(&frame).expect("live frame");
        let reply = match &frame {
            LiveFrame::Entry { space_id, entry } => Reply::new(value).fields([
                ("Space", space_id.clone()),
                ("Change", entry["kind"].as_str().unwrap_or("change").into()),
                ("Item", entry["item"].as_str().unwrap_or("").into()),
                ("Sequence", entry["seq"].to_string()),
                ("Author", entry["author"].as_str().unwrap_or("").into()),
                (
                    "Time",
                    entry["at_ms"]
                        .as_i64()
                        .map(|t| out.local_ms(t))
                        .unwrap_or_default(),
                ),
                ("Details", entry["data"].to_string()),
                ("Request", entry["request_id"].as_str().unwrap_or("").into()),
                ("Bookmark", entry["token"].as_str().unwrap_or("").into()),
            ]),
            LiveFrame::Ready { space_id, bookmark } => Reply::new(value).fields([
                ("Space", space_id.clone()),
                ("Status", "Watching for changes".into()),
                ("Bookmark", bookmark.clone()),
            ]),
            LiveFrame::Reset { .. } => {
                return Err(CliError::new(
                    "conflict",
                    "The bookmark expired. Reload the space before watching again.",
                )
                .hint("Remove this space's bookmark after reloading, then run watch again."));
            }
        };
        if out.event(&reply) != 0 {
            return Err(CliError::new("unavailable", "Cannot write the live event."));
        }
        live.accept(&frame).await.map_err(error)?;
    }
}
fn error(e: calternal_client::ClientError) -> CliError {
    CliError::new(
        serde_json::to_value(e.api_error().code)
            .expect("error code")
            .as_str()
            .expect("string code")
            .to_owned(),
        e.to_string(),
    )
    .hint("Check the server and your access, or run watch --help.")
}
