//! Operator-approved sign-in endpoints for installed harnesses.
use super::reason;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::Path;

// Preserve the distribution's existing sign-in during the package-store
// upgrade. Package bytes, activation approvals and existing revisions are
// deliberately untouched so the preceding release can still read them.
pub(super) const V2: &str = "
CREATE TABLE provider_runtime (
    id TEXT PRIMARY KEY,
    login_command TEXT NOT NULL,
    login_locations TEXT NOT NULL CHECK (json_valid(login_locations))
) STRICT, WITHOUT ROWID;
INSERT INTO provider_runtime VALUES (
    'calternal-claude-code', 'claude auth login',
    '[\"https://claude.ai/oauth/authorize\",\"https://claude.com/oauth/authorize\",
      \"https://platform.claude.com/oauth/authorize\",\"https://console.anthropic.com/oauth/authorize\"]'
);";

/// Approved presentation metadata for a provider's private sign-in relay.
#[derive(Clone)]
pub(crate) struct SignInPolicy {
    /// Human-readable manual sign-in command. The host never executes it.
    pub(crate) command: String,
    locations: Vec<String>,
}

impl SignInPolicy {
    /// Load one indexed registration. Call off the async executor.
    pub(crate) fn read(appdata: &Path, id: &str) -> Result<Option<Self>, String> {
        let db = Connection::open_with_flags(
            appdata.join("installed-plugins.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(reason)?;
        let row: Option<(String, String)> = db
            .query_row(
                "SELECT login_command, login_locations FROM provider_runtime WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(reason)?;
        row.map(|(command, locations)| {
            let locations = serde_json::from_str(&locations).map_err(reason)?;
            Ok(Self { command, locations })
        })
        .transpose()
    }

    /// Validate the official verification address without network traffic.
    /// Pasted-code flows may attach provider-managed OAuth query parameters.
    pub(crate) fn allows(&self, raw: &str, needs_code: bool) -> bool {
        let Ok(url) = raw.parse::<axum::http::Uri>() else {
            return false;
        };
        let Some(authority) = url.authority() else {
            return false;
        };
        if url.scheme_str() != Some("https") || (!needs_code && url.query().is_some()) {
            return false;
        }
        let location = format!("https://{authority}{}", url.path());
        self.locations
            .iter()
            .any(|candidate| *candidate == location)
    }
}
