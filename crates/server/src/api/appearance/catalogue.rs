//! Per-provider refreshes and deterministic, cached merged catalogue snapshots.
use super::remote::{self, Entry};
use crate::store::sha256_hex;
use calternal_core::{FontCatalogueInput, FontCatalogueResult, FontSource};
use calternal_types::Error;
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

#[derive(Default)]
struct Provider {
    rows: Vec<Entry>,
    retry: Option<Instant>,
    stale: bool,
    revision: u64,
}

#[derive(Default)]
struct Snapshot {
    revisions: [u64; 3],
    entries: Vec<Entry>,
}

#[derive(Default)]
pub(super) struct Catalogue {
    sources: [Mutex<Provider>; 3],
    snapshot: Mutex<Arc<Snapshot>>,
}

impl Catalogue {
    async fn refresh(&self, client: &reqwest::Client, index: usize) {
        let mut slot = self.sources[index].lock().await;
        if slot.retry.is_some_and(|later| later > Instant::now()) {
            return;
        }
        match remote::catalogue(client, index).await {
            Ok(rows) => {
                slot.rows = rows;
                slot.revision += 1;
                slot.stale = false;
                slot.retry = Some(Instant::now() + Duration::from_secs(24 * 3600));
            }
            Err(error) => {
                tracing::warn!(index, message=%error.message, "font catalogue refresh failed");
                slot.stale = true;
                slot.retry = Some(Instant::now() + Duration::from_secs(5 * 60));
            }
        }
    }

    async fn snapshot(&self, client: Option<&reqwest::Client>) -> (Arc<Snapshot>, bool) {
        let Some(client) = client else {
            return (self.snapshot.lock().await.clone(), true);
        };
        tokio::join!(
            self.refresh(client, 0),
            self.refresh(client, 1),
            self.refresh(client, 2)
        );
        let slots = [
            self.sources[0].lock().await,
            self.sources[1].lock().await,
            self.sources[2].lock().await,
        ];
        let revisions = slots.each_ref().map(|s| s.revision);
        let mut cached = self.snapshot.lock().await;
        if cached.revisions != revisions {
            let mut used = HashSet::new();
            let entries = slots
                .iter()
                .flat_map(|slot| slot.rows.iter())
                .filter(|entry| used.insert(entry.folded.clone()))
                .cloned()
                .collect();
            *cached = Arc::new(Snapshot { revisions, entries });
        }
        (cached.clone(), slots.iter().any(|slot| slot.stale))
    }

    pub(super) async fn entry(&self, client: &reqwest::Client, id: &str) -> Result<Entry, Error> {
        let (snapshot, _) = self.snapshot(Some(client)).await;
        snapshot
            .entries
            .iter()
            .find(|entry| entry.row.id == id)
            .cloned()
            .ok_or_else(super::missing)
    }

    pub(super) async fn page(
        &self,
        client: Option<&reqwest::Client>,
        input: FontCatalogueInput,
    ) -> Result<FontCatalogueResult, Error> {
        let limit = super::page_size(input.limit)?;
        let query = input.query.as_deref().unwrap_or("").trim();
        if query.chars().count() > 120
            || matches!(input.source, Some(FontSource::Upload | FontSource::System))
        {
            return Err(Error::invalid_input("invalid font catalogue filter"));
        }
        let query = query.to_lowercase();
        let (snapshot, partial) = self.snapshot(client).await;
        let revision = sha256_hex(format!("{:?}", snapshot.revisions).as_bytes());
        let offset = match input.cursor {
            Some(cursor) => cursor
                .strip_prefix(&revision)
                .and_then(|cursor| cursor.strip_prefix(':'))
                .and_then(|offset| offset.parse::<usize>().ok())
                .filter(|offset| *offset <= snapshot.entries.len())
                .ok_or_else(|| {
                    Error::invalid_input("font catalogue refreshed; start from the first page")
                })?,
            None => 0,
        };
        let mut families: Vec<_> = snapshot
            .entries
            .iter()
            .filter(|entry| {
                input.source.is_none_or(|source| source == entry.row.source)
                    && input.mono.is_none_or(|mono| mono == entry.row.mono)
                    && entry.folded.contains(&query)
            })
            .skip(offset)
            .take(limit + 1)
            .map(|entry| entry.row.clone())
            .collect();
        let more = families.len() > limit;
        families.truncate(limit);
        Ok(FontCatalogueResult {
            families,
            next_cursor: more.then(|| format!("{revision}:{}", offset + limit)),
            partial,
        })
    }
}
