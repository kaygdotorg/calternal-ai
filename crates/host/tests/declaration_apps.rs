//! Install and exercise the shipped declaration-only bundles on the real host.
use calternal_auth::{Auth, AuthConfig};
use calternal_host::{Call, Host};
use calternal_types::{AccountId, Actor, Error, ErrorCode, SpaceId};
use serde_json::{Value, json};
use std::path::PathBuf;

const READING: &str = "calternal-reading-list.reading";
const TASK: &str = "calternal-tasks.tasks";

struct Fixture {
    root: tempfile::TempDir,
    auth: Auth,
    host: Host,
    owner: AccountId,
    space: SpaceId,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let auth = Auth::open(
            &root.path().join("auth"),
            AuthConfig::new("http://localhost").unwrap(),
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let owner = runtime.block_on(auth.create("Owner")).unwrap().id;
        let space = auth.spaces().list(owner).unwrap()[0].id;
        let host = Host::new(root.path().to_owned(), auth.clone(), 7).unwrap();
        for plugin in [
            "calternal-chat",
            "calternal-reading-list",
            "calternal-tasks",
        ] {
            let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../plugins-v2")
                .join(plugin);
            host.install_local(owner, space, &directory, true)
                .unwrap_or_else(|error| panic!("{plugin}: {error:?}"));
        }
        Self {
            root,
            auth,
            host,
            owner,
            space,
        }
    }

    fn call(&self, name: &str, input: Value, key: Option<&str>) -> Result<Value, Error> {
        self.host.call(
            Actor::account(self.owner),
            self.space,
            Call {
                call: name.into(),
                input,
                idempotency_key: key.map(str::to_owned),
                slot_selection: None,
            },
        )
    }

    fn write(&self, change: Value) -> Value {
        self.call("s.write", json!({"changes":[change]}), None)
            .unwrap()
    }

    fn add(&self, kind: &str, fields: Value, body: &str) -> Value {
        self.write(json!({"op":"add","kind":kind,"fields":fields,"body":body}))["changes"][0]["id"]
            .clone()
    }

    fn query(&self, input: Value) -> Value {
        self.call("s.query", input, None).unwrap()
    }

    fn read(&self, id: &Value) -> Value {
        self.call("s.read", json!({"item":id}), None).unwrap()
    }

    fn reopen(mut self) -> Self {
        drop(self.host);
        self.host = Host::new(self.root.path().to_owned(), self.auth.clone(), 7).unwrap();
        self
    }
}

#[test]
fn reading_list_projects_and_checks_operations_and_search() {
    let f = Fixture::new();
    let catalog = f.call("s.declarations", Value::Null, None).unwrap();
    let plugins = catalog["plugins"].as_array().unwrap();
    assert_eq!(
        plugins
            .iter()
            .map(|plugin| plugin["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "calternal-chat",
            "calternal-reading-list",
            "calternal-tasks"
        ]
    );
    assert_eq!(
        plugins[1]["kinds"]["reading"]["search"],
        json!(["title", "site", "body"])
    );
    assert_eq!(
        plugins[1]["kinds"]["reading"]["operations"]["remove"]["form"],
        "remove"
    );
    assert_eq!(
        plugins[1]["collections"]["all"]["views"][1]["columns"],
        json!(["read"])
    );
    assert_eq!(plugins[1]["collections"]["unread"]["icon"], "book");
    assert_eq!(
        plugins[2]["kinds"]["tasks"]["operations"]["done"]["assignments"][1]["computed"],
        true
    );

    let invalid = f
        .call(
            "s.write",
            json!({"changes":[{"op":"add","kind":READING,"fields":{
                "url":"javascript:alert(1)", "title":"Invalid"
            }}]}),
            None,
        )
        .unwrap_err();
    assert_eq!(invalid.code, ErrorCode::InvalidInput);
    let id = f.add(
        READING,
        json!({"url":"https://example.test/private-path", "title":"A saved page"}),
        "A memorable excerpt",
    );
    let unread = json!({"collection":"calternal-reading-list.unread"});
    let row = f.query(unread.clone())["rows"][0].clone();
    assert_eq!(row["computed"]["site"], "example.test");
    assert_eq!(row["when"]["mark read"], true);
    assert!(row["fields"]["added"].as_str().is_some());
    for term in ["SAVED", "example.test", "memorable"] {
        assert_eq!(
            f.query(json!({"from":READING,"search":term}))["rows"][0]["id"],
            id
        );
    }
    assert_eq!(
        f.query(json!({"from":READING,"search":"private-path"}))["rows"],
        json!([])
    );

    let mark = json!({"changes":[{"op":"perform","item":id,"operation":"mark read"}]});
    let result = f.call("s.write", mark.clone(), Some("mark-read")).unwrap();
    assert_eq!(
        f.call("s.write", mark.clone(), Some("mark-read")).unwrap(),
        result
    );
    assert!(f.call("s.write", mark, Some("mark-twice")).is_err());
    assert_eq!(f.query(unread)["rows"], json!([]));
    let f = f.reopen();
    assert_eq!(f.read(&id)["when"]["mark read"], false);
    assert_eq!(f.read(&id)["fields"]["read"], true);

    f.write(json!({"op":"change","item":id,"fields":{"title":"Edited title"}}));
    assert_eq!(f.read(&id)["fields"]["title"], "Edited title");
    f.write(json!({"op":"remove","item":id}));
    assert_eq!(
        f.call("s.read", json!({"item":id}), None).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn tasks_complete_with_one_atomic_patch_and_project_recurrences_and_totals() {
    let f = Fixture::new();
    let yesterday = time::OffsetDateTime::now_utc()
        .date()
        .previous_day()
        .unwrap();
    let scheduled = yesterday.to_string();
    let id = f.add(
        TASK,
        json!({
            "title":"A repeating task","scheduled":scheduled,
            "repeat":"FREQ=DAILY", "repeat from":"completion", "tags":["project/blue"]
        }),
        "",
    );
    let inbox = json!({"collection":"calternal-tasks.open"});
    let row = f.query(inbox.clone())["rows"][0].clone();
    assert_eq!(row["fields"]["status"], "todo");
    assert_eq!(row["computed"]["overdue"], true);
    assert_eq!(
        row["computed"]["next"],
        yesterday.next_day().unwrap().to_string()
    );
    assert_eq!(row["when"]["done"], true);
    assert_eq!(
        f.query(json!({"from":TASK,"search":"project/blue"}))["rows"][0]["id"],
        id
    );

    let invalid = f
        .call(
            "s.write",
            json!({"changes":[{"op":"change","item":id,"fields":{
                "repeat":"FREQ=WEEKLY"
            }}]}),
            None,
        )
        .unwrap_err();
    assert_eq!(invalid.code, ErrorCode::Conflict);

    let due = yesterday
        .next_day()
        .unwrap()
        .next_day()
        .unwrap()
        .to_string();
    f.write(json!({"op":"perform","item":id,"operation":"schedule","fields":{"scheduled":due}}));
    assert_eq!(f.read(&id)["fields"]["scheduled"], due);
    let done = json!({"op":"perform","item":id,"operation":"done"});
    assert!(
        f.call("s.write", json!({"changes":[done, done]}), None)
            .is_err()
    );
    assert_eq!(f.read(&id)["fields"]["status"], "todo");
    f.call("s.write", json!({"changes":[done]}), None).unwrap();
    assert_eq!(f.query(inbox)["rows"], json!([]));
    let row = f.read(&id);
    assert_eq!(row["fields"]["status"], "done");
    assert_eq!(row["version"], 3);
    let completed = time::OffsetDateTime::parse(
        row["fields"]["completed"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    let next = row["computed"]["next"].as_str().unwrap();
    assert!(
        next.starts_with(&completed.date().next_day().unwrap().to_string()),
        "{next}"
    );
    assert_eq!(row["when"]["done"], false);
    assert_eq!(row["when"]["won't do"], false);
    f.add(TASK, json!({"title":"Another task"}), "");
    let all = f.query(json!({"collection":"calternal-tasks.all","limit":1}));
    assert_eq!(all["totals"], json!([2]));
    assert_eq!(
        all["groups"],
        json!([
            {"key":"done","matched":1,"totals":[1]},
            {"key":"todo","matched":1,"totals":[1]}
        ])
    );
    assert_eq!(all["rows"][0]["groups"], json!(["done"]));
    let result = f.query(json!({"collection":"calternal-tasks.all","search":"Another"}));
    assert_eq!(result["totals"], json!([1]));
}

#[test]
fn installing_a_line_claim_explains_why_it_cannot_run() {
    let f = Fixture::new();
    let bundle = tempfile::tempdir().unwrap();
    std::fs::create_dir(bundle.path().join("kinds")).unwrap();
    std::fs::write(
        bundle.path().join("plugin.md"),
        "---\nplugin: example-lines\nname: Example lines\nversion: 1.0.0\nformat: 1\n---\nExample.\n",
    ).unwrap();
    let spec = include_str!("../../../docs/proposals/kind-and-collection-format-2026-11-24.md");
    let appendix = spec.split_once("### B.2 Tasks").unwrap().1;
    let document = appendix
        .split_once("```markdown\n")
        .unwrap()
        .1
        .split_once("```")
        .unwrap()
        .0;
    let template = include_str!("../../../plugins-v2/calternal-tasks/kinds/tasks.md");
    let claim = document
        .lines()
        .find(|line| line.starts_with("lines:"))
        .unwrap();
    let document = template
        .replace("folder: Tasks", "folder: Example Lines")
        .replace("editable: outside", &format!("editable: outside\n{claim}"));
    std::fs::write(bundle.path().join("kinds/tasks.md"), document).unwrap();
    let error = f
        .host
        .install_local(f.owner, f.space, bundle.path(), false)
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Forbidden, "{error:?}");
    assert!(error.message.contains("lines in other notes"), "{error:?}");
}
