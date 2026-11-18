//! Bounded, derived full-text projection of each space's current Markdown heads.
mod fuzzy;
mod readers;
mod source;
mod wal;
mod worker;
use crate::{Content, Result, discovery::prefix_end, types::internal};
use calternal_types::{Error, SpaceId};
pub use fuzzy::typo_expression;
pub(super) use readers::{Readers, Writers};
use rusqlite::{Connection, functions::FunctionFlags, params};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const SCHEMA: &str = include_str!("search/schema.sql");

/// One cached manifest kind and the activation control that admits it to search.
#[derive(Serialize)]
pub struct SearchKind {
    /// Manifest kind identifier.
    pub kind: String,
    /// Existing key in per-space controls.
    pub control: String,
    /// Effective value if there is no override.
    pub default_enabled: bool,
    /// Current package approval stamp, required for a stored override.
    pub approval: u32,
    /// Space owning a development draft. An installed package has no restriction.
    pub space: Option<SpaceId>,
}

/// Precompiled search configuration derived from one published manifest catalog.
#[derive(Debug)]
pub struct SearchConfig {
    kinds: String,
    filters: String,
}

impl SearchConfig {
    /// Serialize manifest data once per catalog. Input contains only available,
    /// claimed-text kinds that explicitly opt in.
    pub fn new(mut kinds: Vec<SearchKind>) -> Self {
        kinds.sort_by(|a, b| a.kind.cmp(&b.kind));
        Self {
            kinds: serde_json::json!(kinds.iter().map(|k| &k.kind).collect::<Vec<_>>()).to_string(),
            filters: serde_json::json!(kinds).to_string(),
        }
    }
}

/// Load the searchable projection of the current catalog without I/O.
/// Returning None means the owner closed and the worker should exit.
pub type SearchConfigSource = Box<dyn Fn() -> Option<Arc<SearchConfig>> + Send + 'static>;

/// Title folding for literal index ranges; O(title bytes).
pub fn fold_title(title: &str) -> String {
    title.chars().flat_map(char::to_lowercase).collect()
}

pub(super) fn register_functions(conn: &Connection) -> Result<()> {
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    conn.create_scalar_function("search_fold", 1, flags, |ctx| {
        Ok(fold_title(ctx.get_raw(0).as_str()?))
    })
    .map_err(internal)?;
    conn.create_scalar_function("search_body", 1, flags, |ctx| {
        let bytes = ctx.get_raw(0).as_blob()?;
        let end = std::str::from_utf8(bytes)
            .err()
            .map_or(bytes.len(), |e| e.valid_up_to());
        let text = std::str::from_utf8(&bytes[..end]).expect("valid UTF-8 prefix");
        let body = match text.strip_prefix("---\n") {
            Some(front) => front.split_once("\n---\n").map_or("", |(_, body)| body),
            None => text,
        };
        Ok(body
            .chars()
            .take(8192)
            .filter(|ch| !matches!(ch, '\u{1}' | '\u{2}'))
            .collect::<String>())
    })
    .map_err(internal)?;
    fuzzy::install(conn)
}

/// Stable search key: match tier, folded title (prefix lookup only), item UUID.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SearchKey(i64, String, String);

impl SearchKey {
    /// Read a position authenticated by the server cursor helper.
    pub fn parse(value: Option<&str>) -> Result<Option<Self>> {
        value
            .map(serde_json::from_str)
            .transpose()
            .map_err(|_| Error::invalid_input("invalid search cursor"))
    }

    /// Encode the last delivered key for the server cursor helper.
    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("search key")
    }

    /// Whether this page is continuing the typo fallback.
    pub fn fuzzy(&self) -> bool {
        self.0 == 2
    }
}

/// One result returned by the space's index; the caller binds the space identity.
pub struct IndexedHit {
    /// Global ordering resumes strictly after this key.
    pub key: SearchKey,
    /// Item UUID in canonical form.
    pub id: String,
    /// Manifest kind identifier.
    pub kind: String,
    /// Display title.
    pub title: String,
    /// Plain excerpt with internal highlight delimiters U+0001 and U+0002.
    pub excerpt: String,
}

/// Per-space rows with a coverage flag. Callers combine only authorized pages.
pub struct IndexedPage {
    /// At most the requested bounded count, including caller's lookahead.
    pub hits: Vec<IndexedHit>,
    /// True once this content space exceeds the indexed head slot ceiling.
    pub partial: bool,
}

/// Validated and compiled once per request for all selected spaces.
#[derive(Clone, Copy)]
pub enum SearchMatch<'a> {
    /// Literal prefix over the ordered title index.
    Prefix(&'a str),
    /// Quoted FTS5 prefix expressions for all words and title-only words.
    Text {
        /// Literal prefix query over both searchable columns.
        all: &'a str,
        /// Equivalent query limited to the title column.
        title: &'a str,
    },
    /// Single-word typo fallback: bigram candidates checked by edit distance.
    Fuzzy {
        /// Bigram disjunction.
        grams: &'a str,
        /// User's folded term.
        term: &'a str,
    },
}

/// Request shared by all space index probes.
pub struct SearchProbe<'a> {
    /// One of the three index query modes.
    pub matching: SearchMatch<'a>,
    /// JSON array of wanted kinds, empty for all manifest-enabled kinds.
    pub kinds: &'a str,
    /// Authenticated key from the preceding page.
    pub after: Option<&'a SearchKey>,
    /// Upper bound including lookahead.
    pub limit: usize,
}

// Pass the same activation check to all index reads. A missing control uses the
// registry default. An invalid stored override contributes no results.
const CTE: &str = "WITH allowed(kind) AS (
    SELECT value FROM json_each(?1) WHERE json_array_length(?2)>=0
)";
const PREFIX: &str = "SELECT 0,d.title_key,d.id,d.kind,d.title,'' FROM search_docs d
    WHERE d.kind IN allowed AND (d.title_key,d.id)>(?3,?4) AND d.title_key>=?5
    AND d.title_key<?6 ORDER BY d.title_key,d.id LIMIT ?7";
const MATCH: &str = "WITH
    title_rows AS MATERIALIZED (SELECT rowid FROM search_text WHERE search_text MATCH ?4),
    selected AS MATERIALIZED (
        SELECT CASE WHEN t.rowid IS NULL THEN 1 ELSE 0 END AS tier,
               d.rowid,d.id,d.kind,d.title
        FROM search_text JOIN search_docs d ON d.rowid=search_text.rowid
        LEFT JOIN title_rows t ON t.rowid=d.rowid
        WHERE search_text MATCH ?3 AND d.kind IN allowed
          AND (CASE WHEN t.rowid IS NULL THEN 1 ELSE 0 END,d.id)>(?5,?6)
        ORDER BY tier,d.id LIMIT ?7
    )
    SELECT tier,'',id,kind,title,(
        SELECT CASE WHEN selected.tier=0 THEN highlight(search_text,0,char(1),char(2))
                    ELSE snippet(search_text,1,char(1),char(2),'…',16) END
        FROM search_text WHERE rowid=selected.rowid AND search_text MATCH ?3
    )
    FROM selected";
const FUZZY: &str = "
    SELECT 2,'',d.id,d.kind,d.title,char(1)||d.title||char(2)
    FROM search_title_grams JOIN search_docs d ON d.rowid=search_title_grams.rowid
    WHERE search_title_grams MATCH ?3 AND d.kind IN allowed AND d.id>?4
      AND search_one_edit(d.title_key,?5)
    ORDER BY d.id LIMIT ?6";

fn statements() -> &'static [String; 4] {
    static SQL: std::sync::OnceLock<[String; 4]> = std::sync::OnceLock::new();
    SQL.get_or_init(|| {
        let tail = PREFIX.replace("d.title_key<?6", "?6 IS NULL");
        [PREFIX, &tail, MATCH, FUZZY].map(|query| {
            format!(
                "{CTE}, matches AS MATERIALIZED ({query})
             SELECT * FROM matches ORDER BY 1,2,3"
            )
        })
    })
}

fn search(conn: &Connection, filters: &str, probe: &SearchProbe<'_>) -> Result<IndexedPage> {
    let after = probe.after;
    let id = after.map_or("", |key| key.2.as_str());
    let limit = probe.limit.min(51) as i64;
    let sql = statements();
    let rows = |query, params: &[&dyn rusqlite::ToSql]| -> Result<IndexedPage> {
        let mut stmt = conn.prepare_cached(query).map_err(internal)?;
        let mut cursor = stmt.query(params).map_err(internal)?;
        let mut page = IndexedPage::empty();
        while let Some(r) = cursor.next().map_err(internal)? {
            if let Some(id) = r.get::<_, Option<String>>(2).map_err(internal)? {
                page.hits.push(IndexedHit {
                    key: SearchKey(
                        r.get(0).map_err(internal)?,
                        r.get(1).map_err(internal)?,
                        id.clone(),
                    ),
                    id,
                    kind: r.get(3).map_err(internal)?,
                    title: r.get(4).map_err(internal)?,
                    excerpt: r.get(5).map_err(internal)?,
                });
            }
        }
        Ok(page)
    };
    match probe.matching {
        SearchMatch::Prefix(prefix) => {
            let first = after.map_or(prefix, |key| key.1.as_str());
            let end = prefix_end(prefix);
            rows(
                &sql[usize::from(end.is_none())],
                params![filters, probe.kinds, first, id, prefix, end, limit],
            )
        }
        SearchMatch::Text { all, title } => {
            let tier = after.map_or(0, |key| key.0);
            rows(
                &sql[2],
                params![filters, probe.kinds, all, title, tier, id, limit],
            )
        }
        SearchMatch::Fuzzy { grams, term } => rows(
            &sql[3],
            params![filters, probe.kinds, grams, id, term, limit],
        ),
    }
}

impl IndexedPage {
    fn empty() -> Self {
        Self {
            hits: Vec::new(),
            partial: false,
        }
    }
}

impl Content {
    /// Start one background worker for lazy per-space projections. Startup only
    /// spawns a thread. The owner can join the returned handle after closing the
    /// catalog source. A second configuration attempt returns an error.
    pub fn configure_search(
        self: &Arc<Self>,
        source: SearchConfigSource,
    ) -> Result<std::thread::JoinHandle<()>> {
        let (send, handle) = worker::start(self, source)?;
        self.search_worker
            .set(send)
            .map_err(|_| Error::internal("search worker is already configured"))?;
        Ok(handle)
    }

    /// Query an authorized space's projection. Cold and lagging indexes set
    /// partial and enqueue a bounded background wake. This path never opens
    /// the content writer or runs a rebuild.
    pub fn search_page(
        &self,
        config: &SearchConfig,
        space: SpaceId,
        probe: &SearchProbe<'_>,
    ) -> Result<IndexedPage> {
        let _step = self.lifecycle.step(space.as_uuid())?;
        let path = crate::db::space_path(&self.roots.appdata, space);
        if !path.is_file() || config.kinds == "[]" {
            return Ok(IndexedPage::empty());
        }
        let pair = self.search_readers.take(space, &self.roots.appdata)?;
        let (source, index, journal) = &pair;
        let result = (|| {
            let (allowed, capped) = source::allowed(source, &config.filters, probe.kinds, space)?;
            if allowed == "[]" {
                return Ok(IndexedPage::empty());
            }
            if source::pending(source)? {
                if let Some(worker) = self.search_worker.get() {
                    let _ = worker.try_send(space);
                }
                return Ok(IndexedPage {
                    partial: true,
                    ..IndexedPage::empty()
                });
            }
            let head = journal
                .as_ref()
                .map(|reader| reader.head())
                .transpose()
                .map_err(internal)?;
            let snapshot = index
                .as_ref()
                .map(|conn| readers::while_available(|| conn.unchecked_transaction()))
                .transpose()?
                .flatten();
            let ready = snapshot
                .as_deref()
                .map(|conn| readers::while_available(|| worker::state(conn)))
                .transpose()?
                .flatten()
                .flatten();
            let matches = head
                .as_ref()
                .zip(ready.as_ref())
                .filter(|(head, ready)| ready.ready && ready.matches(config, space, head));
            if matches.is_none_or(|(head, ready)| head.seq != ready.watermark) {
                if let Some(worker) = self.search_worker.get() {
                    let _ = worker.try_send(space);
                }
            }
            let Some((head, ready)) = matches else {
                return Ok(IndexedPage {
                    partial: true,
                    ..IndexedPage::empty()
                });
            };
            let mut page = search(snapshot.as_deref().expect("ready index"), &allowed, probe)?;
            page.partial = capped || head.seq != ready.watermark;
            Ok(page)
        })();
        self.search_readers.keep(space, pair);
        result
    }
}
