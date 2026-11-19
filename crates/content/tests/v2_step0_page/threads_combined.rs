//! One ranked Threads window; title FTS and message blocks share budgets.
use super::*;
use rusqlite::params_from_iter;

const SHAPES: [(Shape, &str); 5] = [
    (Shape::One, "analysis"),
    (Shape::Two, "analysis analytic"),
    (Shape::ThreeSelective, "analysis analytic marker"),
    (Shape::Prefix, "analy"),
    (Shape::Broad, "the"),
];
const TITLE_MATCH: &str =
    "SELECT rowid FROM search_text WHERE search_text MATCH ?1 ORDER BY rowid LIMIT ?2";

#[derive(Debug, PartialEq, Eq)]
pub(in super::super) struct Combined {
    rows: Vec<String>,
    titles: usize,
    bound: usize,
    decoded: usize,
    work: usize,
    refine: bool,
}

pub(in super::super) fn title_projection(conn: &Connection) {
    conn.execute_batch("BEGIN IMMEDIATE;
         INSERT INTO search_text(rowid,title,body)
         SELECT rowid,title ||
           CASE WHEN rowid%1907=0 THEN ' analysis analytic marker'
                WHEN rowid%1809=0 THEN ' analysis analytic'
                WHEN rowid%1913=0 THEN ' analysis' ELSE '' END, ''
         FROM search_docs;
         COMMIT;
         INSERT INTO search_text(search_text) VALUES('optimize');
         CREATE VIRTUAL TABLE title_vocab USING fts5vocab(search_text, 'row');
         CREATE TABLE title_terms(word TEXT PRIMARY KEY, doc INTEGER NOT NULL) STRICT, WITHOUT ROWID;
         INSERT INTO title_terms SELECT term,doc FROM title_vocab;").unwrap();
}

fn title_plan(conn: &Connection, input: &str, prefix: bool) -> (String, usize, Vec<String>) {
    let words = message_terms(input).collect::<Vec<_>>();
    let stats = if prefix {
        conn.prepare_cached(
            "SELECT word,doc FROM title_terms WHERE word>='analy' AND word<'analz'
           ORDER BY word LIMIT 33",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<(String, i64)>>()
    } else {
        let mut q = conn
            .prepare_cached("SELECT coalesce((SELECT doc FROM title_terms WHERE word=?1),0)")
            .unwrap();
        words
            .iter()
            .map(|word| (word.clone(), q.query_row([word], |r| r.get(0)).unwrap()))
            .collect()
    };
    let expression = if prefix {
        format!("{input:?}*")
    } else {
        words
            .iter()
            .map(|word| format!("{word:?}"))
            .collect::<Vec<_>>()
            .join(" AND ")
    };
    let bound = stats.iter().map(|(_, n)| *n as usize).sum();
    (
        expression,
        bound,
        stats.into_iter().map(|(word, _)| word).collect(),
    )
}

fn title_slots(conn: &Connection, expression: &str, target: usize) -> Vec<i64> {
    conn.prepare_cached(TITLE_MATCH)
        .unwrap()
        .query_map(params![expression, target as i64], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn read_all(conn: &Connection, shape: Shape, input: &str, depth: usize) -> Combined {
    let tx = conn.unchecked_transaction().unwrap();
    let prefix = matches!(shape, Shape::Prefix);
    let target = depth + WINDOW;
    let (expression, title_bound, mut expansions) = title_plan(&tx, input, prefix);
    let terms = if prefix {
        shape.plan(&tx)
    } else {
        literal_plan(&tx, input)
    };
    if prefix {
        let args = std::iter::repeat_n("?", terms.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT word FROM terms WHERE id IN ({args})");
        let mut q = tx.prepare_cached(&sql).unwrap();
        expansions.extend(
            q.query_map(params_from_iter(terms.iter().map(|t| t.0)), |r| r.get(0))
                .unwrap()
                .map(Result::unwrap),
        );
        expansions.sort_unstable();
        expansions.dedup();
    }
    let bound = title_bound + terms.iter().map(|t| t.1).sum::<usize>();
    let mut out = Combined {
        rows: vec![],
        titles: 0,
        bound,
        decoded: 0,
        work: 0,
        refine: false,
    };
    if bound > BUDGET || (prefix && expansions.len() > 32) {
        out.refine = true;
        // A broad prefix starts neither expansion. A literal can safely keep already issued rows.
        return out;
    }
    let titles = title_slots(&tx, &expression, target);
    out.work = titles.len();
    out.titles = titles.len();
    let mut docs = tx
        .prepare_cached("SELECT id FROM search_docs WHERE rowid=?1")
        .unwrap();
    for &slot in titles.iter().skip(depth).take(WINDOW) {
        out.rows.push(docs.query_row([slot], |r| r.get(0)).unwrap());
        out.work += 1;
    }
    if titles.len() < target {
        let exclude = titles.iter().copied().collect();
        let remaining_offset = depth.saturating_sub(titles.len());
        let remaining_size = target - titles.len().max(depth);
        let messages = ranked_messages(
            &tx,
            &terms,
            prefix,
            remaining_offset,
            remaining_size,
            &exclude,
            BUDGET - title_bound,
            ROW_BUDGET - out.work,
        );
        out.rows.extend(messages.rows);
        out.decoded = messages.postings;
        out.work += messages.lookups + messages.blocks + messages.descriptors;
        out.refine = messages.refine;
    }
    assert!(out.work <= ROW_BUDGET && out.decoded + title_bound <= BUDGET);
    out
}

fn expected_all(conn: &Connection, shape: Shape, input: &str, depth: usize) -> Vec<String> {
    let prefix = matches!(shape, Shape::Prefix);
    let (expression, _, _) = title_plan(conn, input, prefix);
    let titles = title_slots(conn, &expression, THREADS);
    let message_ids = expected(shape, 0);
    let title_ids = titles
        .iter()
        .map(|slot| id(*slot as usize - 2))
        .collect::<Vec<_>>();
    let mut rank = title_ids.clone();
    // The existing baseline helper returns exactly one window. Use the same
    // independent oracle at successive offsets until the combined window fills.
    let seen = title_ids.into_iter().collect::<HashSet<_>>();
    for offset in (0..=800).step_by(WINDOW) {
        let ids = if offset == 0 {
            message_ids.clone()
        } else {
            expected(shape, offset)
        };
        rank.extend(ids.into_iter().filter(|id| !seen.contains(id)));
        if rank.len() >= depth + WINDOW {
            break;
        }
    }
    rank.into_iter().skip(depth).take(WINDOW).collect()
}

pub(in super::super) fn exercise(root: &Path) {
    let paths = [
        root.join("combined-before.db"),
        root.join("combined-after.db"),
    ];
    for (i, path) in paths.iter().enumerate() {
        seed(path, i == 1);
        let conn = Connection::open(path).unwrap();
        title_projection(&conn);
    }
    let conn = read(&paths[0]);
    let after = read(&paths[1]);
    assert_eq!(state(&conn), state(&after));
    for (shape, input) in SHAPES {
        for depth in [0, 200] {
            let result = read_all(&conn, shape, input, depth);
            assert_eq!(read_all(&after, shape, input, depth), result);
            if matches!(shape, Shape::Broad) {
                assert!(result.refine && result.rows.is_empty());
            } else {
                assert!(!result.refine);
                assert_eq!(result.rows, expected_all(&conn, shape, input, depth));
                assert_eq!(result.rows.len(), WINDOW);
                assert!(result.titles > 0 && result.titles < depth + WINDOW && result.decoded > 0);
            }
            let mut samples = Vec::new();
            for _ in 0..RUNS {
                let start = Instant::now();
                let actual = read_all(&read(&paths[0]), shape, input, depth);
                samples.push(start.elapsed().as_micros());
                assert_eq!(actual, result);
            }
            println!(
                "all_threads shape={shape:?} depth={depth} n={RUNS} bound={} decoded={} title_matches={} row_work={} rows={} refine={} us {}",
                result.bound,
                result.decoded,
                result.titles,
                result.work,
                result.rows.len(),
                result.refine,
                summary(&mut samples)
            );
        }
    }
    println!("all_threads hidden_rank_changes=0 hidden_state_changes=0");
}

pub(in super::super) fn contains_append(conn: &Connection, marker: &str, id: &str) {
    let window = read_all(conn, Shape::One, marker, 0);
    assert!(!window.refine);
    assert_eq!(window.rows, [id]);
}

pub(in super::super) fn visible_window(conn: &Connection) -> Combined {
    read_all(conn, Shape::Prefix, "analy", 0)
}
