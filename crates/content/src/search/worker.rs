//! One bounded maintenance lane, independent of request read/write admission.
#[cfg(test)]
mod tests;
use super::{
    MessageDecoder, Result, SCHEMA, SearchConfig, SearchConfigSource, internal, messages,
    register_functions, source, wal,
};
use crate::{Content, journal::lock};
use calternal_store::{ProjectionJournal, SyncToken};
use calternal_types::SpaceId;
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc, Weak,
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};

const POLL: Duration = Duration::from_millis(50);
const PAGE: usize = 128;
const REQUESTS: usize = 128;
/// Schema version of the per-space search database and its file name.
const VERSION: i64 = 2;
const MIGRATIONS: calternal_store::migrations::Schema<'_> =
    calternal_store::migrations::Schema::new("search projection", VERSION, &[SCHEMA], &[]);
/// Projection generation. Bump it when what the rows mean changes, as generation 3
/// did when thread bodies moved to the message index. A mismatch rebuilds the space.
const GENERATION: &str = "3";

pub(super) fn start(
    content: &Arc<Content>,
    source: SearchConfigSource,
    decode: MessageDecoder,
) -> Result<(SyncSender<(SpaceId, bool)>, std::thread::JoinHandle<()>)> {
    let (send, receive) = mpsc::sync_channel(REQUESTS);
    let weak = Arc::downgrade(content);
    let handle = std::thread::Builder::new()
        .name("search-index".into())
        .spawn(move || work(weak, receive, source, decode))
        .map_err(internal)?;
    Ok((send, handle))
}

fn work(
    content: Weak<Content>,
    receive: Receiver<(SpaceId, bool)>,
    source: SearchConfigSource,
    decode: MessageDecoder,
) {
    let mut queue = VecDeque::new();
    let mut marked = HashSet::new();
    let mut windows = HashSet::new();
    let mut ready = HashMap::new();
    let mut more = false;
    let mut previous: Option<Arc<SearchConfig>> = None;
    loop {
        if queue.is_empty() && !more {
            match receive.recv_timeout(POLL) {
                Ok((space, create_window)) => {
                    ready.remove(&space);
                    add(&mut queue, &mut marked, space);
                    if create_window {
                        windows.insert(space);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        let Some(config) = source() else {
            return;
        };
        let Some(content) = content.upgrade() else {
            return;
        };
        let pin = lock(&content.root_guard).clone();
        if pin.is_none() {
            std::thread::sleep(POLL);
            continue;
        }
        if previous
            .as_ref()
            .is_none_or(|prior| prior.kinds != config.kinds)
        {
            ready.clear();
        }
        previous = Some(Arc::clone(&config));
        let active: Vec<_> = lock(&content.spaces).keys().copied().collect();
        ready.retain(|space, _| active.contains(space));
        for space in active {
            add(&mut queue, &mut marked, space);
        }
        for (space, create_window) in receive.try_iter().take(REQUESTS) {
            ready.remove(&space);
            add(&mut queue, &mut marked, space);
            if create_window && marked.contains(&space) {
                windows.insert(space);
            }
        }
        more = false;
        let jobs = queue.len();
        for _ in 0..jobs {
            let Some(space) = queue.pop_front() else {
                break;
            };
            marked.remove(&space);
            let create_window = windows.remove(&space);
            match content.advance_window(space, create_window) {
                Ok(true) => {
                    more = true;
                    add(&mut queue, &mut marked, space);
                    if create_window {
                        windows.insert(space);
                    }
                }
                Ok(false) => {}
                Err(error) => tracing::debug!(%space, ?error, "window projection will retry"),
            }
            if config.kinds == "[]" {
                continue;
            }
            match project(&content, &config, space, ready.get(&space), decode) {
                Ok(Progress::Ready(head)) => {
                    ready.insert(space, head);
                }
                Ok(Progress::More) => {
                    more = true;
                    add(&mut queue, &mut marked, space);
                }
                Ok(Progress::Idle) => {}
                Err(error) => tracing::debug!(%space, ?error, "search projection will retry"),
            }
        }
    }
}

fn add(queue: &mut VecDeque<SpaceId>, marked: &mut HashSet<SpaceId>, space: SpaceId) {
    if queue.len() < REQUESTS && marked.insert(space) {
        queue.push_back(space);
    }
}

pub(super) fn db_path(appdata: &std::path::Path, space: SpaceId) -> std::path::PathBuf {
    crate::db::space_path(appdata, space).with_file_name(format!("search-v{VERSION}.db"))
}

enum Progress {
    Idle,
    More,
    Ready(SyncToken),
}

fn project(
    content: &Content,
    config: &SearchConfig,
    space: SpaceId,
    known: Option<&SyncToken>,
    decode: MessageDecoder,
) -> Result<Progress> {
    let _step = content.lifecycle.step(space.as_uuid())?;
    if let Some(known) = known {
        let slot = lock(&content.spaces).get(&space).cloned();
        if let Some(slot) = slot
            && let Ok(held) = slot.space.try_lock()
            && let Some(open) = held.as_ref()
            && &open.journal.head() == known
        {
            return Ok(Progress::Idle);
        }
    }
    let source_path = crate::db::space_path(&content.roots.appdata, space);
    if !source_path.is_file() {
        return Ok(Progress::Idle);
    }
    // A stopped process can leave an unretired content intent without a new
    // journal head. A window cache slot can exist before the authoritative
    // store has opened. Visibility is installed only after cold recovery.
    let recovered = lock(&content.spaces)
        .get(&space)
        .is_some_and(|slot| slot.visibility.get().is_some());
    if !recovered {
        let source = crate::pool::open_reader(&source_path).map_err(internal)?;
        if source::pending(&source)? {
            content.live_journal(space)?;
        }
    }
    let Some(journal) = ProjectionJournal::open(&content.roots.appdata, space).map_err(internal)?
    else {
        return Ok(Progress::Idle);
    };
    let head = journal.head().map_err(internal)?;
    if known == Some(&head) {
        return Ok(Progress::Idle);
    }
    let finish = |progress| {
        if !config.has_messages() {
            return Ok(progress);
        }
        match messages::advance(content, space, &journal, &head, &source_path, decode)? {
            messages::Progress::Ready => Ok(progress),
            messages::Progress::More => Ok(Progress::More),
            messages::Progress::Idle => Ok(match progress {
                Progress::Ready(_) => Progress::Idle,
                p => p,
            }),
        }
    };
    let path = db_path(&content.roots.appdata, space);
    if path.exists() {
        let on_disk = crate::pool::open_reader(&path).map_err(internal)?;
        on_disk.busy_timeout(POLL).map_err(internal)?;
        if state(&on_disk)
            .map_err(internal)?
            .is_some_and(|s| s.ready && s.matches(config, space, &head) && s.watermark == head.seq)
        {
            return finish(Progress::Ready(head.clone()));
        }
    }
    let mut index = match content.search_writers.take(space) {
        Some(index) => index,
        None => open_writer(&path)?,
    };
    let result = checkpoint_if_needed(&index, &path).and_then(|room| {
        if room {
            advance(
                content,
                config,
                space,
                &journal,
                &head,
                &source_path,
                &mut index,
            )
        } else {
            Ok(Progress::Idle)
        }
    });
    content.search_writers.keep(space, index);
    result.and_then(finish)
}

fn open_writer(path: &std::path::Path) -> Result<Connection> {
    calternal_fsio::private::file(path).map_err(internal)?;
    let index = wal::open(path).map_err(internal)?;
    index.busy_timeout(POLL).map_err(internal)?;
    index
        .execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-4096;",
        )
        .map_err(internal)?;
    register_functions(&index)?;
    calternal_store::migrations::migrate(&index, MIGRATIONS).map_err(internal)?;
    Ok(index)
}

pub(super) fn checkpoint_if_needed(index: &Connection, path: &std::path::Path) -> Result<bool> {
    let bytes = match std::fs::metadata(path.with_extension("db-wal")) {
        Ok(info) => info.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(internal(error)),
    };
    if bytes < wal::CHECKPOINT_AT {
        return Ok(true);
    }
    // Runs only on the maintenance thread. A reader that pins the WAL leaves
    // this turn idle. Foreground work can continue and the worker retries.
    index
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|busy| busy == 0)
        .map_err(internal)
}

fn advance(
    content: &Content,
    config: &SearchConfig,
    space: SpaceId,
    journal: &ProjectionJournal,
    head: &SyncToken,
    source_path: &std::path::Path,
    index: &mut Connection,
) -> Result<Progress> {
    let mut current = match state(index).map_err(internal)? {
        Some(state) if state.matches(config, space, head) => state,
        _ => reset(index, config, space, head)?,
    };
    let mut source = crate::pool::open_reader(source_path).map_err(internal)?;
    if let Some(after) = current.scan_after {
        // An empty space can have content metadata before it has a userdata
        // directory. Only the content writer provisions that private directory.
        let folder = calternal_fsio::beneath::Folder::open_existing(&content.roots.userdata_ssd)
            .and_then(|root| root.open_sub(&format!("spaces/{space}"), false))
            .ok();
        let snapshot = source.transaction().map_err(internal)?;
        let heads = source::scan(&snapshot, folder.as_ref(), &config.kinds, after, PAGE)?;
        snapshot.commit().map_err(internal)?;
        current.scan_after = if heads.len() == PAGE {
            heads.last().map(|document| document.slot)
        } else {
            None
        };
        let tx = index.transaction().map_err(internal)?;
        for document in heads {
            replace(&tx, &document.id, Some(&document))?;
        }
        save_state(&tx, &current)?;
        tx.commit().map_err(internal)?;
        return Ok(Progress::More);
    }
    if !replay(journal, &mut source, index, config, &mut current)? {
        return Ok(Progress::Idle);
    }
    let head = journal.head().map_err(internal)?;
    if current.ready && current.matches(config, space, &head) && current.watermark == head.seq {
        Ok(Progress::Ready(head))
    } else {
        Ok(Progress::More)
    }
}

fn replay(
    journal: &ProjectionJournal,
    source: &mut Connection,
    index: &mut Connection,
    config: &SearchConfig,
    state: &mut State,
) -> Result<bool> {
    let head = journal.head().map_err(internal)?;
    let changes = if head.seq == state.watermark {
        Vec::new()
    } else {
        journal.since(state.watermark, PAGE).map_err(internal)?
    };
    let snapshot = source.transaction().map_err(internal)?;
    if source::pending(&snapshot)? {
        return Ok(false);
    }
    let tx = index.transaction().map_err(internal)?;
    for entry in changes {
        if let Some(item) = entry.item {
            let current = source::get(&snapshot, &config.kinds, &item)?;
            replace(&tx, &item, current.as_ref())?;
        }
        state.watermark = entry.sequence;
    }
    if !state.ready && state.watermark == journal.head().map_err(internal)?.seq {
        state.ready = true;
    }
    save_state(&tx, state)?;
    tx.commit().map_err(internal)?;
    Ok(true)
}

fn replace(index: &Connection, id: &str, document: Option<&source::Head>) -> Result<()> {
    if let Some(row) = index
        .prepare_cached("SELECT rowid FROM search_docs WHERE id=?1")
        .map_err(internal)?
        .query_row([id], |row| row.get::<_, i64>(0))
        .optional()
        .map_err(internal)?
    {
        for sql in [
            "DELETE FROM search_title_grams WHERE rowid=?1",
            "DELETE FROM search_text WHERE rowid=?1",
            "DELETE FROM search_docs WHERE rowid=?1",
        ] {
            index
                .prepare_cached(sql)
                .map_err(internal)?
                .execute([row])
                .map_err(internal)?;
        }
    }
    if let Some(doc) = document {
        index
            .prepare_cached("INSERT INTO search_docs VALUES(?1,?2,?3,?4,search_fold(?4))")
            .map_err(internal)?
            .execute(params![doc.slot, doc.id, doc.kind, doc.title])
            .map_err(internal)?;
        index
            .prepare_cached(
                "INSERT INTO search_text(rowid,title,body) VALUES(?1,?2,search_body(?3))",
            )
            .map_err(internal)?
            .execute(params![
                doc.slot,
                doc.title,
                if doc.kind == "threads" {
                    &[][..]
                } else {
                    doc.bytes.as_slice()
                }
            ])
            .map_err(internal)?;
        index.prepare_cached("INSERT INTO search_title_grams(rowid,grams) VALUES(?1,search_grams(search_fold(?2)))")
            .map_err(internal)?.execute(params![doc.slot, doc.title]).map_err(internal)?;
    }
    Ok(())
}

pub(super) struct State {
    kinds: String,
    space: String,
    epoch: String,
    generation: String,
    pub(super) watermark: i64,
    scan_after: Option<i64>,
    pub(super) ready: bool,
}
impl State {
    pub(super) fn matches(&self, config: &SearchConfig, space: SpaceId, head: &SyncToken) -> bool {
        self.generation == GENERATION
            && self.kinds == config.kinds
            && self.space == space.to_string()
            && self.epoch == head.epoch
            && self.watermark <= head.seq
    }
}
pub(super) fn state(index: &Connection) -> rusqlite::Result<Option<State>> {
    let initialized: bool = index.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='search_configuration')",
        [],
        |row| row.get(0),
    )?;
    if !initialized {
        return Ok(None);
    }
    index.prepare_cached("SELECT kinds,space,epoch,generation,watermark,scan_after,ready FROM search_configuration")
        ?.query_row([], |row| Ok(State {
            kinds: row.get(0)?, space: row.get(1)?, epoch: row.get(2)?,
            generation: row.get(3)?, watermark: row.get(4)?, scan_after: row.get(5)?, ready: row.get(6)?,
        })).optional()
}
fn reset(
    index: &mut Connection,
    config: &SearchConfig,
    space: SpaceId,
    head: &SyncToken,
) -> Result<State> {
    let tx = index.transaction().map_err(internal)?;
    tx.execute_batch(
        "DROP TABLE search_text; DROP TABLE search_title_grams;
           DELETE FROM search_docs; DELETE FROM search_configuration;",
    )
    .map_err(internal)?;
    tx.execute_batch(SCHEMA).map_err(internal)?;
    tx.execute(
        "INSERT INTO search_configuration VALUES(1,?1,?2,?3,?4,?5,0,0)",
        params![
            config.kinds,
            space.to_string(),
            head.epoch,
            GENERATION,
            head.seq
        ],
    )
    .map_err(internal)?;
    tx.commit().map_err(internal)?;
    Ok(State {
        kinds: config.kinds.clone(),
        space: space.to_string(),
        epoch: head.epoch.clone(),
        generation: GENERATION.to_owned(),
        watermark: head.seq,
        scan_after: Some(0),
        ready: false,
    })
}
fn save_state(index: &Connection, state: &State) -> Result<()> {
    index
        .execute(
            "UPDATE search_configuration SET watermark=?1,scan_after=?2,ready=?3 WHERE id=1",
            params![state.watermark, state.scan_after, state.ready],
        )
        .map_err(internal)?;
    Ok(())
}
