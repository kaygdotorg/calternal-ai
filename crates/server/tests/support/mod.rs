//! Shared fixtures for plugin tests: protocol-v2 test workers written in plain
//! JavaScript (no SDK), so each test controls exactly what the worker sends.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use calternal_core::ManifestV1;
use calternal_server::plugins::{Plugin, TrustedEntry, file_sha256};
use serde_json::json;

/// The shipped notes-insights manifest, parsed by the real host library.
pub fn insights_manifest() -> ManifestV1 {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/notes-insights/manifest.json");
    calternal_plugin_host::parse_manifest(&std::fs::read(path).unwrap()).unwrap()
}

/// A manifest from inline JSON with every list defaulted to empty.
pub fn manifest(extra: serde_json::Value) -> ManifestV1 {
    let mut base = json!({
        "id": "fixture", "name": "Fixture", "icon": "puzzle", "version": "0.1.0", "api_version": 1,
        "kinds": [], "operations": [], "collections": [], "views": [], "attention": [],
        "settings": [], "schedules": [], "webhooks": [], "secrets": [], "hooks": [],
        "themes": [], "host_calls": []
    });
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    serde_json::from_value(base).unwrap()
}

/// Trusted entry for `manifest` whose worker is `worker` (digest pinned).
pub fn plugin(manifest: ManifestV1, worker: Option<&Path>, default_enabled: bool) -> Plugin {
    let entry = TrustedEntry {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        manifest: "manifest.json".into(),
        worker: worker.map(|_| "worker.js".into()),
        worker_sha256: worker.map(|w| file_sha256(w).unwrap()),
        default_enabled,
    };
    Plugin::new(
        entry,
        manifest,
        worker.map(|w| std::fs::canonicalize(w).unwrap()),
    )
    .unwrap()
}

/// Write a v2 test worker to `dir/worker.js`.
///
/// `on_invoke` runs with `f` (the frame), `m` (its message) and `send(frame)`;
/// `on_reply` additionally sees the stored invoke message as `inv`. Helpers:
/// `call(f, method, input)` sends a host call; `done(f, data)` completes;
/// `stats(text)` builds a notes-insights result for `text`.
pub fn worker(dir: &Path, on_invoke: &str, on_reply: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("worker.js");
    let script = format!(
        r#"const fs=require('fs');const rl=require('readline').createInterface({{input:process.stdin}});
const send=f=>process.stdout.write(JSON.stringify(f)+'\n');
const inv=new Map();
const frame=(f,message)=>({{protocol_version:2,invocation_id:f.invocation_id,message}});
const call=(f,method,input)=>send(frame(f,{{type:'host_call',call_id:'c'+Math.random().toString(36).slice(2,8),method,input,authority:inv.get(f.invocation_id).authority}}));
const done=(f,data)=>send(frame(f,{{type:'complete',data}}));
const stats=t=>{{const s={{words:t.split(/\s+/).filter(Boolean).length,characters:[...t].length,lines:t.split('\n').filter((l,i,a)=>i<a.length-1||l).length}};return {{stats:s,view:{{genui_version:1,root:{{type:'stack',id:'root',children:[{{type:'stat',id:'w',label:'Words',value:String(s.words)}}]}}}}}}}};
rl.on('line',l=>{{const f=JSON.parse(l);const m=f.message;
 if(m.type==='invoke'){{inv.set(f.invocation_id,m);{on_invoke}}}
 else if(m.type==='host_reply'){{const i=inv.get(f.invocation_id);{on_reply}}}
}});"#
    );
    std::fs::write(&path, script).unwrap();
    path
}

/// Default behaviour: read the note through the broker, then return its statistics.
pub const READ_NOTE: &str = "call(f,'read_item',{kind:'notes',item_id:m.input.id})";
/// Default reply handler: complete with statistics of the item's text.
pub const COMPLETE: &str = "done(f,stats(m.ok?m.data.text:''))";
/// Reply handler that waits for a `go` file next to the worker before completing.
pub const COMPLETE_AFTER_GO: &str = "const t=setInterval(()=>{if(fs.existsSync(__dirname+'/go')){clearInterval(t);done(f,stats(m.ok?m.data.text:''))}},10)";

/// The development fixture accounts and spaces, found the way a client finds them:
/// each dev token authenticates, then `describe` lists the account's spaces. The
/// guest belongs only to the shared space; the owner's other space is personal.
#[derive(Debug, Clone, Copy)]
pub struct Dev {
    pub owner: calternal_server::auth::Identity,
    pub guest: calternal_server::auth::Identity,
    pub personal: calternal_types::SpaceId,
    pub shared: calternal_types::SpaceId,
}

impl Dev {
    /// The fixture identity for a test-side account name, `owner` or `guest`.
    pub fn who(&self, name: &str) -> calternal_server::auth::Identity {
        match name {
            "owner" => self.owner,
            "guest" => self.guest,
            other => panic!("unknown fixture account {other}"),
        }
    }
}

/// Read [`Dev`] from a development directory. Runs on its own thread because the
/// directory blocks on its database worker.
pub fn dev(table: &calternal_server::auth::TokenTable) -> Dev {
    use calternal_server::auth::Directory;
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let identity = |name: &str| {
                    let (_, path) = table.files.iter().find(|(n, _)| *n == name).unwrap();
                    table.authenticate(&std::fs::read(path).unwrap()).unwrap()
                };
                let (owner, guest) = (identity("owner"), identity("guest"));
                let spaces = |who| {
                    table
                        .describe(&who)
                        .spaces
                        .into_iter()
                        .map(|g| calternal_types::SpaceId::parse(&g.id).unwrap())
                        .collect::<Vec<_>>()
                };
                let shared = spaces(guest)[0];
                let personal = spaces(owner).into_iter().find(|s| *s != shared).unwrap();
                Dev {
                    owner,
                    guest,
                    personal,
                    shared,
                }
            })
            .join()
            .unwrap()
    })
}

/// Three sibling roots under one disposable directory.
pub fn roots(dir: &Path) -> calternal_roots::Roots {
    calternal_roots::Roots {
        appdata: dir.join("appdata"),
        userdata_ssd: dir.join("userdata-ssd"),
        userdata_hdd: dir.join("userdata-hdd"),
    }
}
