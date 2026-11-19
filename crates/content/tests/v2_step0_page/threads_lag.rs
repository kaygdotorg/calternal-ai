//! Append acknowledgement to real journal-backed message search publication.
use super::{
    RUNS, summary,
    threads_index::{publish_append, seed, version, visible_snapshot},
};
use calternal_content::{
    Content, FileFormat, Item, MessageRecord, Placement, Request, SearchConfig, SearchKind,
    SearchMatch, SearchProbe, message_terms,
};
use calternal_roots::Roots;
use calternal_types::{AccountId, SpaceId};
use rusqlite::{Connection, params};
use serde_json::json;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const POLL: Duration = Duration::from_millis(2);
const TIMEOUT: Duration = Duration::from_secs(15);
const BUSY: usize = 6;

fn decode(bytes: &[u8]) -> calternal_content::Result<Vec<MessageRecord>> {
    let text = std::str::from_utf8(bytes).unwrap();
    let body = text
        .strip_prefix("---\n")
        .map_or(text, |rest| rest.split_once("\n---\n").unwrap().1);
    let mut at = text.len() - body.len();
    let mut records = Vec::new();
    for literal in body.split_inclusive("\n\n\n") {
        let (heading, body) = literal.split_once(" -->\n\n").unwrap();
        let id = Uuid::parse_str(heading.strip_prefix("<!-- ").unwrap()).unwrap();
        let body_start = at + heading.len() + " -->\n\n".len();
        let end = at + literal.len();
        records.push(MessageRecord {
            id,
            stamp: (id.as_u128() as u64) as i64,
            offset_seconds: 0,
            record: at..end,
            body: body_start..body_start + body.len(),
        });
        at = end;
    }
    Ok(records)
}

fn register(content: &Content, space: SpaceId) {
    content
        .register_kind(
            space,
            "threads",
            Placement::Visible {
                folder: "Threads".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
}

fn append(
    content: &Content,
    space: SpaceId,
    actor: AccountId,
    old: Option<&Item>,
    seq: usize,
    text: &str,
) -> Item {
    let id = Uuid::from_u128(10_000_000 + seq as u128);
    let mut body = old
        .map(|item| item.bytes.clone())
        .unwrap_or_else(|| b"---\nformat: fixture-thread-v1\n---\n".to_vec());
    body.extend_from_slice(format!("<!-- {id} -->\n\n{text}\n\n\n").as_bytes());
    let key = format!("append-{seq}");
    content
        .write(
            space,
            old.map(|item| item.id),
            "threads",
            old.map(|item| item.revision.as_str()),
            json!({"title":"Lag fixture","message_count":seq}),
            &body,
            Request {
                account: actor,
                via: None,
                id: &key,
                operation: "threads.append",
                fingerprint: &key,
            },
            |_| json!({}),
        )
        .unwrap()
}

fn query(content: &Content, config: &SearchConfig, space: SpaceId, term: &str) -> (bool, bool) {
    let terms = message_terms(term).collect::<Vec<_>>();
    let page = content
        .search_page(
            config,
            space,
            &SearchProbe {
                matching: SearchMatch::Text {
                    all: &format!("{term:?}"),
                    title: "title:never",
                    message_terms: &terms,
                    phrase: false,
                    messages_only: true,
                },
                kinds: r#"["threads"]"#,
                item_ids: None,
                after: None,
                limit: 3,
            },
        )
        .unwrap();
    (page.partial, !page.hits.is_empty())
}

fn settled(content: &Content, config: &SearchConfig, space: SpaceId, term: &str) {
    let start = Instant::now();
    loop {
        if query(content, config, space, term) == (false, true) {
            return;
        }
        assert!(
            start.elapsed() < TIMEOUT,
            "message worker did not publish {term}"
        );
        thread::sleep(POLL);
    }
}

pub(super) fn exercise(root: &Path) {
    let roots = Roots {
        appdata: root.join("appdata"),
        userdata_ssd: root.join("ssd"),
        userdata_hdd: root.join("hdd"),
    };
    let guards = calternal_roots::prepare(&roots, Uuid::now_v7(), true).unwrap();
    let appdata = roots.appdata.clone();
    let content = Arc::new(
        Content::open(
            roots,
            root.join("secrets"),
            Duration::from_secs(120),
            calternal_content::DEFAULT_RECEIPT_RETENTION,
            |_| {},
        )
        .unwrap(),
    );
    content.retain_root_guard(Arc::new(guards));
    let config = Arc::new(SearchConfig::new(vec![SearchKind {
        kind: "threads".into(),
        control: "fixture".into(),
        default_enabled: true,
        approval: 0,
        space: None,
    }]));
    let space = SpaceId::new();
    register(&content, space);
    let actor = AccountId::new();
    let mut prior = append(&content, space, actor, None, 0, "initial anchor");
    content.materialize(space, prior.id).unwrap();
    // Seed the ready message schema at the real journal watermark. Subsequent
    // appends use the real worker's replacement and watermark transactions.
    let base = appdata.join(format!("spaces/{space}/search-messages-v1.all.db"));
    let (_, corpus, _) = seed(&base, false);
    let scope_path = root.join("scope-live.db");
    std::fs::copy(&base, &scope_path).unwrap();
    let mut scope = Connection::open(scope_path).unwrap();
    scope
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
        .unwrap();
    let combined = std::env::var_os("STEP0_ALL_SEARCH").is_some();
    if combined {
        super::threads_index::combined::title_projection(&scope);
    }
    let token = content.live_journal(space).unwrap().head();
    let baseline = Connection::open(&base).unwrap();
    let next: i64 = baseline
        .query_row("SELECT max(id)+1 FROM blocks", [], |r| r.get(0))
        .unwrap();
    baseline
        .execute(
            "INSERT INTO build_state VALUES(1,'2',?1,?2,NULL,1,?3)",
            params![token.epoch, token.seq, next],
        )
        .unwrap();
    drop(baseline);
    let worker = content
        .configure_search(
            Box::new({
                let cfg = config.clone();
                move || Some(cfg.clone())
            }),
            decode,
        )
        .unwrap();
    prior = append(&content, space, actor, Some(&prior), 1, "warm anchor");
    settled(&content, &config, space, "warm");
    content.materialize(space, prior.id).unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let churn = Arc::new(AtomicUsize::new(0));
    let mut producers = Vec::new();
    let mut seq = 1;
    let reps = std::env::var("STEP0_LAG_RUNS").map_or(RUNS, |v| v.parse().unwrap());
    assert!(reps > 0);
    for busy in [false, true] {
        if busy {
            for _ in 0..BUSY {
                let other = SpaceId::new();
                register(&content, other);
                let content = content.clone();
                let cfg = config.clone();
                let stop = stop.clone();
                let churn = churn.clone();
                producers.push(thread::spawn(move || {
                    let actor = AccountId::new();
                    let mut prior = None;
                    let mut seq = 0;
                    while !stop.load(Ordering::Acquire) {
                        prior = Some(append(
                            &content,
                            other,
                            actor,
                            prior.as_ref(),
                            seq,
                            "busy update review agenda documents",
                        ));
                        content
                            .materialize(other, prior.as_ref().unwrap().id)
                            .unwrap();
                        let _ = query(&content, &cfg, other, "busy");
                        churn.fetch_add(1, Ordering::Release);
                        seq += 1;
                        thread::sleep(Duration::from_millis(20));
                    }
                }));
            }
            let start = Instant::now();
            while churn.load(Ordering::Acquire) < BUSY * 4 {
                assert!(start.elapsed() < TIMEOUT);
                thread::sleep(POLL);
            }
        }
        let initial = churn.load(Ordering::Acquire);
        let mut core = Vec::new();
        let mut scoped = Vec::new();
        let mut finished = Vec::new();
        for _ in 0..reps {
            seq += 1;
            let marker = format!("appendmarker{seq}");
            let body = format!(
                "{marker} The review now includes analysis of the final content and the updated project agenda."
            );
            let last = version(&scope);
            prior = append(&content, space, actor, Some(&prior), seq, &body);
            let ack = Instant::now();
            assert_eq!(version(&scope), last); // Bind shows loading while this visible change is pending.
            settled(&content, &config, space, &marker);
            core.push(ack.elapsed().as_micros());
            publish_append(&mut scope, &prior.id.to_string(), &body, seq);
            let found: i64 = scope
                .query_row(
                    "SELECT s.doc FROM term_stats s JOIN terms t ON t.id=s.term WHERE t.word=?1",
                    [&marker],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!((found, version(&scope)), (1, (last.0, last.1 + 1)));
            scoped.push(ack.elapsed().as_micros());
            if combined {
                super::threads_index::combined::contains_append(
                    &scope,
                    &marker,
                    &prior.id.to_string(),
                );
                finished.push(ack.elapsed().as_micros());
            }
            content.materialize(space, prior.id).unwrap();
        }
        if combined {
            println!(
                "all_threads_lag busy={busy} n={reps} ready_us {}",
                summary(&mut finished)
            );
        }
        println!(
            "thread_lag busy={busy} n={reps} seeded_messages={corpus} competing_saves={} core_us {} scoped_us {}",
            churn.load(Ordering::Acquire) - initial,
            summary(&mut core),
            summary(&mut scoped)
        );
    }
    stop.store(true, Ordering::Release);
    for p in producers {
        p.join().unwrap();
    }
    // In the same real space, the upstream worker sees the hidden commit.
    // Its global head/partial status is never the reader's scoped state.
    let hidden = append(&content, space, actor, None, 9_999, "initialhidden thread");
    content.materialize(space, hidden.id).unwrap();
    settled(&content, &config, space, "initialhidden");
    let before = visible_snapshot(&scope);
    let all_before = combined.then(|| super::threads_index::combined::visible_window(&scope));
    let hidden = append(
        &content,
        space,
        actor,
        Some(&hidden),
        10_000,
        "privateonly hidden update analysis",
    );
    content.materialize(space, hidden.id).unwrap();
    settled(&content, &config, space, "privateonly");
    assert_eq!(visible_snapshot(&scope), before);
    if combined {
        assert_eq!(
            Some(super::threads_index::combined::visible_window(&scope)),
            all_before
        );
    }
    assert_eq!(
        scope
            .query_row(
                "SELECT count(*) FROM terms WHERE word='privateonly'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    println!(
        "thread_hidden_live worker_published=1 scoped_state_changed=0 scoped_ranks_changed=0 scoped_private_terms=0"
    );
    drop(content);
    worker.join().unwrap();
}
