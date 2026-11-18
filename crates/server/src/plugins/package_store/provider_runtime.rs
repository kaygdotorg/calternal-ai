//! Operator-approved sign-in endpoints for installed harnesses.
use super::reason;
use calternal_core::{HarnessOffer as Offer, HarnessPin as Pin};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::Path;

// Preserve the distribution's existing sign-in during the package-store
// upgrade. Package bytes, activation approvals and existing revisions are
// deliberately untouched so the preceding release can still read them.
pub(super) const V2: &str = "
CREATE TABLE provider_runtime (
    id TEXT PRIMARY KEY,
    login_command TEXT NOT NULL,
    login_locations TEXT NOT NULL CHECK (json_valid(login_locations)),
    legacy INTEGER NOT NULL DEFAULT 0 CHECK (legacy IN (0, 1)),
    active_pin TEXT CHECK (active_pin IS NULL OR json_valid(active_pin)),
    previous_pin TEXT CHECK (previous_pin IS NULL OR json_valid(previous_pin)),
    tested_pin TEXT CHECK (tested_pin IS NULL OR json_valid(tested_pin)),
    tested_revision INTEGER
) STRICT, WITHOUT ROWID;
INSERT INTO provider_runtime (id, login_command, login_locations, legacy) VALUES (
    'calternal-claude-code', 'claude auth login',
    '[\"https://claude.ai/oauth/authorize\",\"https://claude.com/oauth/authorize\",
      \"https://platform.claude.com/oauth/authorize\",\"https://console.anthropic.com/oauth/authorize\"]', 1
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

fn validate_pin(pin: &Pin) -> Result<(), String> {
    let hash = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    if pin.version.is_empty()
        || pin.version.len() > 64
        || !pin
            .version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(&byte))
        || !hash(&pin.package_sha256)
        || !pin.image.strip_prefix("sha256:").is_some_and(hash)
        || pin
            .guest_image_digest
            .as_deref()
            .is_some_and(|digest| !hash(digest))
    {
        return Err("invalid provider runtime version or digest".into());
    }
    Ok(())
}

pub(crate) fn read_pin(appdata: &Path, id: &str) -> Result<Option<(Pin, bool)>, String> {
    let db = Connection::open_with_flags(
        appdata.join("installed-plugins.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(reason)?;
    let row: Option<(bool, Option<String>)> = db
        .query_row(
            "SELECT legacy, active_pin FROM provider_runtime WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(reason)?;
    match row {
        Some((legacy, Some(json))) => {
            let offer: Offer = serde_json::from_str(&json).map_err(reason)?;
            validate_offer(&offer)?;
            Ok(Some((offer.pin, legacy)))
        }
        Some((true, None)) => Ok(None),
        _ => Err("No tested provider runtime is active. Check the Admin harness list.".into()),
    }
}

pub(crate) fn sandbox(
    pin: &Pin,
    provider_id: String,
    login_scope: calternal_machine::protocol::ProviderLoginScope,
    workspace_read_only: bool,
) -> calternal_machine::protocol::HarnessSandbox {
    calternal_machine::protocol::HarnessSandbox {
        provider_id,
        login_scope,
        workspace_read_only,
        disposable_profile: false,
        legacy_profile: false,
        guest_image_digest: pin.guest_image_digest.clone(),
    }
}

pub(crate) fn validate_offer(offer: &Offer) -> Result<(), String> {
    validate_pin(&offer.pin)?;
    if offer.login_command.len() > 128
        || offer.login_command.chars().any(char::is_control)
        || offer.login_locations.len() > 8
        || offer.login_locations.iter().any(|location| {
            location.len() > 256
                || !location.parse::<axum::http::Uri>().is_ok_and(|url| {
                    url.scheme_str() == Some("https")
                        && url.authority().is_some_and(|authority| {
                            !authority.as_str().contains('@') && authority.port().is_none()
                        })
                        && url.query().is_none()
                })
        })
    {
        return Err("invalid provider sign-in metadata".into());
    }
    Ok(())
}

pub(crate) fn stage(appdata: &Path, id: &str, revision: u32, offer: Offer) -> Result<(), String> {
    validate_offer(&offer)?;
    let db = super::open(appdata)?;
    let staged = serde_json::to_string(&offer).map_err(reason)?;
    let locations = serde_json::to_string(&offer.login_locations).map_err(reason)?;
    let changed = db.execute(
        "INSERT INTO provider_runtime (id, login_command, login_locations, tested_pin, tested_revision)
         SELECT id, ?3, ?4, ?5, revision FROM installed_plugins
         WHERE id = ?1 AND revision = ?2 AND json_type(CAST(archive AS TEXT), '$.manifest.provider') = 'object'
         ON CONFLICT (id) DO UPDATE SET
           tested_pin = excluded.tested_pin, tested_revision = excluded.tested_revision",
        rusqlite::params![id, revision, offer.login_command, locations, staged],
    ).map_err(reason)?;
    if changed != 1 {
        return Err("the provider package changed during the smoke test".into());
    }
    Ok(())
}

pub(crate) fn list(appdata: &Path) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let db = super::open(appdata)?;
    let mut query = db
        .prepare(
            "SELECT p.id, json_extract(p.archive, '$.manifest.name'),
                COALESCE(r.legacy, 0), active_pin, previous_pin,
                CASE WHEN tested_revision = p.revision THEN tested_pin END
         FROM installed_plugins p LEFT JOIN provider_runtime r ON r.id = p.id
         WHERE json_type(CAST(p.archive AS TEXT), '$.manifest.provider') = 'object'
         ORDER BY p.id",
        )
        .map_err(reason)?;
    let mut rows = query.query([]).map_err(reason)?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().map_err(reason)? {
        let decode = |index| -> Result<Option<Pin>, String> {
            row.get::<_, Option<String>>(index)
                .map_err(reason)?
                .map(|json| {
                    serde_json::from_str::<Offer>(&json)
                        .map(|offer| offer.pin)
                        .map_err(reason)
                })
                .transpose()
        };
        result.push(json!({
            "id": row.get::<_, String>(0).map_err(reason)?,
            "name": row.get::<_, String>(1).map_err(reason)?,
            "bundled": row.get::<_, bool>(2).map_err(reason)?,
            "active": decode(3)?,
            "previous": decode(4)?,
            "tested": decode(5)?,
        }));
    }
    Ok(json!({"harnesses": result}))
}

pub(crate) fn select(appdata: &Path, id: &str, image: &str, rollback: bool) -> Result<Pin, String> {
    let mut db = super::open(appdata)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(reason)?;
    let (active, candidate): (Option<String>, Option<String>) = tx
        .query_row(
            "SELECT r.active_pin,
          CASE WHEN ?2 THEN r.previous_pin
               WHEN r.tested_revision = p.revision THEN r.tested_pin END
         FROM provider_runtime r JOIN installed_plugins p ON p.id = r.id WHERE r.id = ?1",
            rusqlite::params![id, rollback],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(reason)?
        .ok_or("harness is not registered")?;
    if let Some(active) = &active {
        let current: Offer = serde_json::from_str(active).map_err(reason)?;
        if current.pin.image == image {
            return Ok(current.pin);
        }
    }
    let raw = candidate.ok_or("No matching smoke-tested runtime is available.")?;
    let offer: Offer = serde_json::from_str(&raw).map_err(reason)?;
    validate_offer(&offer)?;
    if offer.pin.image != image {
        return Err("The offered runtime changed. Reload the harness list.".into());
    }
    let locations = serde_json::to_string(&offer.login_locations).map_err(reason)?;
    tx.execute(
        "UPDATE provider_runtime SET
         previous_pin = active_pin, active_pin = ?2, login_command = ?3, login_locations = ?4,
         tested_pin = CASE WHEN ?5 THEN tested_pin ELSE NULL END,
         tested_revision = CASE WHEN ?5 THEN tested_revision ELSE NULL END
         WHERE id = ?1",
        rusqlite::params![id, raw, offer.login_command, locations, rollback],
    )
    .map_err(reason)?;
    tx.commit().map_err(reason)?;
    Ok(offer.pin)
}
