//! Exercise the Notes tokenizer and existing scope-local posting budget.
use super::{POSTING_BUDGET, RUNS, SCHEMA, WINDOW, fold_title, read, summary, text_ids};
use rusqlite::{Connection, params};
use std::{path::Path, time::Instant};

const NATIVE: &str = "unicode61 remove_diacritics 2";
const MARKS: &str = "unicode61 remove_diacritics 2 categories 'L* N* Co M*'";

struct Arm {
    name: &'static str,
    terms: [&'static str; 3],
    query: [&'static str; 3],
    prefix: &'static str,
    title_prefix: &'static str,
    marks: bool,
    contiguous: bool,
}

const ARMS: &[Arm] = &[
    Arm {
        name: "latin",
        terms: ["RÉSUMÉ", "CAFÉ", "ÉCOLE"],
        query: ["resume", "cafe", "ecole"],
        prefix: "resu",
        title_prefix: "RÉSU",
        marks: false,
        contiguous: false,
    },
    Arm {
        name: "cyrillic",
        terms: ["ЁЛКА", "МОСКВА", "БИБЛИОТЕКА"],
        query: ["ёлка", "москва", "библиотека"],
        prefix: "ЁЛ",
        title_prefix: "ЁЛ",
        marks: false,
        contiguous: false,
    },
    Arm {
        name: "devanagari_default",
        terms: ["किताब", "दिल्ली", "भारत"],
        query: ["किताब", "दिल्ली", "भारत"],
        prefix: "कि",
        title_prefix: "कि",
        marks: false,
        contiguous: false,
    },
    Arm {
        name: "devanagari_marks",
        terms: ["किताब", "दिल्ली", "भारत"],
        query: ["किताब", "दिल्ली", "भारत"],
        prefix: "कि",
        title_prefix: "कि",
        marks: true,
        contiguous: false,
    },
    Arm {
        name: "han_spaced",
        terms: ["资料", "会议", "记录"],
        query: ["资料", "会议", "记录"],
        prefix: "资料",
        title_prefix: "资料",
        marks: false,
        contiguous: false,
    },
    Arm {
        name: "han_unspaced",
        terms: ["资料", "会议", "记录"],
        query: ["资料", "会议", "记录"],
        prefix: "资料",
        title_prefix: "资料",
        marks: false,
        contiguous: true,
    },
    Arm {
        name: "japanese_unspaced",
        terms: ["資料", "かいぎ", "記録"],
        query: ["資料", "かいぎ", "記録"],
        prefix: "資料",
        title_prefix: "資料",
        marks: false,
        contiguous: true,
    },
    Arm {
        name: "hangul_unspaced",
        terms: ["자료", "회의", "기록"],
        query: ["자료", "회의", "기록"],
        prefix: "자료",
        title_prefix: "자료",
        marks: false,
        contiguous: true,
    },
];

fn schema(arm: &Arm) -> String {
    if arm.marks {
        SCHEMA.replace(NATIVE, &MARKS.replace('\'', "''"))
    } else {
        SCHEMA.to_owned()
    }
}

// Use the deployed SQLite tokenizer in the settled-query compiler, not Rust
// char classification. The scratch table receives at most one short literal.
fn tokenizer(arms: &Arm) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(&schema(arms)).unwrap();
    c.execute_batch("CREATE VIRTUAL TABLE instances USING fts5vocab(search_text,'instance');")
        .unwrap();
    c
}

fn tokens(c: &Connection, input: &str) -> Vec<String> {
    c.execute("DELETE FROM search_text", []).unwrap();
    c.execute("INSERT INTO search_text(title,body) VALUES('',?1)", [input])
        .unwrap();
    c.prepare("SELECT term FROM instances ORDER BY offset")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn quoted(input: &str) -> String {
    format!("\"{}\"", input.replace('"', "\"\""))
}

fn end(prefix: &str) -> String {
    let (at, last) = prefix.char_indices().next_back().unwrap();
    format!(
        "{}{}",
        &prefix[..at],
        char::from_u32(last as u32 + 1).unwrap()
    )
}

fn seed(source: &Path, path: &Path, arm: &Arm, collection: bool) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(&schema(arm)).unwrap();
    conn.execute("ATTACH DATABASE ?1 AS source", [source.to_str().unwrap()])
        .unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    let sql = "INSERT INTO search_docs
        SELECT row_number() OVER (ORDER BY d.id),d.id,'notes',
             CASE WHEN ((d.rowid-1)/10)%100<38 THEN ?1 ELSE 'memo' END,
             CASE WHEN ((d.rowid-1)/10)%100<38 THEN ?2 ELSE 'memo' END
        FROM source.search_docs d JOIN source.acl a ON a.id=d.id
        WHERE a.visible AND (NOT ?3 OR EXISTS (SELECT 1 FROM source.note_collection_members m
            WHERE m.collection_id='collection' AND m.note_id=d.id))";
    conn.execute(
        sql,
        params![arm.terms[0], fold_title(arm.terms[0]), collection],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO search_text(rowid,title,body)
         SELECT d.rowid,d.title, CASE WHEN ?4 THEN
             CASE WHEN ((s.rowid-1)/10)%100 BETWEEN 20 AND 54 THEN ?1||?2||?3 ELSE 'memo' END
           ELSE
             CASE WHEN ((s.rowid-1)/10)%100<55 THEN ?1 ELSE 'memo' END || ' ' ||
             CASE WHEN ((s.rowid-1)/10)%100 BETWEEN 10 AND 59 THEN ?2 ELSE 'memo' END || ' ' ||
             CASE WHEN ((s.rowid-1)/10)%100 BETWEEN 20 AND 69 THEN ?3 ELSE 'memo' END
           END
         FROM search_docs d JOIN source.search_docs s ON s.id=d.id",
        params![arm.terms[0], arm.terms[1], arm.terms[2], arm.contiguous],
    )
    .unwrap();
    conn.execute_batch(
        "COMMIT;
         INSERT INTO search_text(search_text) VALUES('optimize');
         CREATE VIRTUAL TABLE vocab USING fts5vocab(search_text,'row');
         CREATE TABLE term_stats(term TEXT PRIMARY KEY,doc INTEGER NOT NULL)
           WITHOUT ROWID,STRICT;
         INSERT INTO term_stats SELECT term,doc FROM vocab;",
    )
    .unwrap();
}

enum Mode {
    And,
    Token,
    Title,
    Run,
}
impl Mode {
    fn name(&self) -> &'static str {
        match self {
            Self::And => "three",
            Self::Token => "token_prefix",
            Self::Title => "title_prefix",
            Self::Run => "whole_run",
        }
    }
}

struct Compiled {
    expression: String,
    stats: Vec<String>,
    prefix: Option<String>,
}

fn scope_work(conn: &Connection, mode: &Mode, plan: &Compiled) -> (i64, usize) {
    if matches!(mode, Mode::Title) {
        return (0, 0);
    }
    if let Some(prefix) = &plan.prefix {
        let mut query = conn
            .prepare_cached("SELECT doc FROM term_stats WHERE term>=?1 AND term<?2 LIMIT 33")
            .unwrap();
        let docs = query
            .query_map(params![prefix, end(prefix)], |r| r.get::<_, i64>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        return (docs.iter().sum(), docs.len());
    }
    let mut query = conn
        .prepare_cached("SELECT coalesce((SELECT doc FROM term_stats WHERE term=?1),0)")
        .unwrap();
    let mut work = 0;
    for term in &plan.stats {
        work += query.query_row([term], |r| r.get::<_, i64>(0)).unwrap();
    }
    (work, plan.stats.len())
}

fn ids(conn: &Connection, depth: i64, mode: &Mode, plan: &Compiled) -> Vec<String> {
    if matches!(mode, Mode::Title) {
        return conn
            .prepare_cached(
                "SELECT id FROM search_docs WHERE kind='notes' AND title_key>=?1 AND title_key<?2
            ORDER BY title_key,id LIMIT ?3 OFFSET ?4",
            )
            .unwrap()
            .query_map(
                params![plan.expression, end(&plan.expression), WINDOW as i64, depth],
                |r| r.get(0),
            )
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
    }
    text_ids(conn, &plan.expression, depth, None)
}

fn expected_ids(
    conn: &Connection,
    collection: bool,
    arm: &Arm,
    mode: &Mode,
    depth: i64,
) -> Vec<String> {
    let band = match mode {
        Mode::And if arm.contiguous => "0",
        Mode::And | Mode::Run => "band BETWEEN 20 AND 54",
        Mode::Token => "band<55",
        Mode::Title => "band<38",
    };
    let sql = format!(
        "WITH visible AS (
      SELECT d.id,((d.rowid-1)/10)%100 AS band FROM search_docs d JOIN acl a ON a.id=d.id
      WHERE a.visible AND (NOT ?1 OR EXISTS (SELECT 1 FROM note_collection_members m
         WHERE m.collection_id='collection' AND m.note_id=d.id))
    ) SELECT id FROM visible WHERE {band} ORDER BY id LIMIT ?2 OFFSET ?3"
    );
    conn.prepare(&sql)
        .unwrap()
        .query_map(params![collection, WINDOW as i64, depth], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn correctness(c: &Connection, arm: &Arm) {
    let hit = |text: &str, expression: &str| {
        c.execute("DELETE FROM search_text", []).unwrap();
        c.execute("INSERT INTO search_text(title,body) VALUES('',?1)", [text])
            .unwrap();
        c.query_row(
            "SELECT EXISTS(SELECT 1 FROM search_text WHERE search_text MATCH ?1)",
            [expression],
            |r| r.get::<_, bool>(0),
        )
        .unwrap()
    };
    if arm.name == "latin" || arm.marks {
        assert!(hit("RÉSUMÉ", &quoted("resume")));
        assert!(hit("RE\u{301}SUME\u{301}", &quoted("resume")));
        assert!(hit("résumé", &format!("{}*", quoted("RESU"))));
        assert!(!fold_title("RÉSUMÉ").starts_with("resu"));
    }
    if arm.name == "cyrillic" || arm.marks {
        assert!(hit("ЁЛКА", &quoted("ёлка")));
        assert!(!hit("ЁЛКА", &quoted("елка")));
    }
    if arm.name.starts_with("devanagari") {
        assert!(hit("किताब", &quoted("किताब")));
        assert_eq!(hit("कुताब", &quoted("किताब")), !arm.marks);
        assert_eq!(tokens(c, "किताब").len() == 1, arm.marks);
    } else if arm.contiguous {
        let joined = arm.terms.concat();
        assert_eq!(tokens(c, &joined).len(), 1);
        assert!(hit(&joined, &quoted(&joined)));
        assert!(!hit(&joined, &quoted(arm.terms[1])));
        assert!(hit(&joined, &format!("{}*", quoted(arm.terms[0]))));
    }
}

pub(crate) fn exercise(source: &Path, scratch: &Path) {
    let source_read = read(source);
    for arm in ARMS {
        let compiler = tokenizer(arm);
        correctness(&compiler, arm);
        let actual = arm.query.map(|word| tokens(&compiler, word));
        let prefix_tokens = tokens(&compiler, arm.prefix);
        println!(
            "i18n_tokenizer arm={} words={actual:?} prefix={prefix_tokens:?}",
            arm.name
        );
        let joined = arm.terms.concat();
        let mut queries = vec![
            (
                Mode::And,
                Compiled {
                    expression: arm.query.map(quoted).join(" AND "),
                    stats: actual.into_iter().flatten().collect(),
                    prefix: None,
                },
            ),
            (
                Mode::Token,
                Compiled {
                    expression: format!("{}*", quoted(arm.prefix)),
                    stats: Vec::new(),
                    prefix: prefix_tokens.last().cloned(),
                },
            ),
            (
                Mode::Title,
                Compiled {
                    expression: fold_title(arm.title_prefix),
                    stats: Vec::new(),
                    prefix: None,
                },
            ),
        ];
        if arm.contiguous {
            queries.push((
                Mode::Run,
                Compiled {
                    expression: quoted(&joined),
                    stats: tokens(&compiler, &joined),
                    prefix: None,
                },
            ));
        }
        for collection in [false, true] {
            let path = scratch.join("international.db");
            seed(source, &path, arm, collection);
            let verify = read(&path);
            for (mode, plan) in &queries {
                let (postings, expansions) = scope_work(&verify, mode, plan);
                let admitted = postings <= POSTING_BUDGET && expansions <= 32;
                let expected_matches = if admitted && !matches!(mode, Mode::Title) {
                    Some(
                        verify
                            .query_row(
                                "SELECT count(*) FROM search_text WHERE search_text MATCH ?1",
                                [&plan.expression],
                                |r| r.get::<_, i64>(0),
                            )
                            .unwrap(),
                    )
                } else {
                    None
                };
                if let Some(count) = expected_matches {
                    if arm.contiguous && matches!(mode, Mode::And) {
                        assert_eq!(count, 0);
                    } else {
                        assert!(count >= 2200, "{} {}: {count}", arm.name, mode.name());
                    }
                }
                for depth in [0, 200, 2_000] {
                    let expected = admitted.then(|| ids(&verify, depth, mode, plan));
                    if let Some(rows) = &expected {
                        assert_eq!(
                            *rows,
                            expected_ids(&source_read, collection, arm, mode, depth)
                        );
                    }
                    let mut times = Vec::new();
                    for _ in 0..RUNS {
                        let start = Instant::now();
                        let conn = read(&path);
                        let txn = conn.unchecked_transaction().unwrap();
                        let (work, terms) = scope_work(&txn, mode, plan);
                        let actual = (work <= POSTING_BUDGET && terms <= 32)
                            .then(|| ids(&txn, depth, mode, plan));
                        times.push(start.elapsed().as_micros());
                        assert_eq!(actual, expected);
                    }
                    println!(
                        "i18n arm={} collection={collection} mode={} depth={depth} n={RUNS} postings={postings} matches={expected_matches:?} rows={:?} us {}",
                        arm.name,
                        mode.name(),
                        expected.as_ref().map(Vec::len),
                        summary(&mut times)
                    );
                }
            }
            drop(verify);
            std::fs::remove_file(path).unwrap();
        }
    }
}
