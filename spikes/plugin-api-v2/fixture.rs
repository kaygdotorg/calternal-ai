//! Disposable access to the real Notes, Threads and membership engines.
use calternal_server::{
    api::{AppState, Shared, dispatch},
    auth::{Identity, StoredDirectory},
    plugins::{Jobs, Registry, Session, WorkerPool},
    store::Store,
};
use calternal_types::Error;
use serde_json::Value;
use std::{path::Path, sync::Arc};
use uuid::Uuid;

pub(crate) fn open(
    roots: &calternal_roots::Roots,
    packages: &Path,
) -> (Shared, calternal_auth::spaces::DevelopmentIds) {
    let store = Store::open(
        roots,
        Uuid::from_u128(1),
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let directory = StoredDirectory::load_or_create(&roots.appdata).unwrap();
    let ids = spaces(roots).seed_development().unwrap();
    let state = Arc::new(AppState {
        blocking: calternal_server::blocking::RequestWorkers::new().unwrap(),
        inputs: store.account_inputs(),
        store,
        directory,
        registry: Registry::load(packages),
        workers: WorkerPool::new(None),
        jobs: Jobs::open(&roots.appdata).unwrap(),
        allowed_hosts: calternal_server::api::allowed_hosts(8080),
    });
    (state, ids)
}

pub(crate) fn roots(path: &Path) -> calternal_roots::Roots {
    calternal_roots::Roots {
        appdata: path.join("appdata"),
        userdata_ssd: path.join("ssd"),
        userdata_hdd: path.join("hdd"),
    }
}

pub(crate) fn spaces(roots: &calternal_roots::Roots) -> calternal_auth::spaces::Spaces {
    calternal_auth::spaces::Spaces::open(&roots.appdata.join("system/auth.db")).unwrap()
}

pub(crate) async fn run(
    state: &Shared,
    who: Identity,
    op: &str,
    input: Value,
) -> Result<Value, Error> {
    dispatch(
        state.clone(),
        who,
        Session::Static,
        &Uuid::new_v4().to_string(),
        op,
        serde_json::from_value(input).unwrap(),
    )
    .await
}

pub(crate) fn identity(id: calternal_types::AccountId) -> Identity {
    Identity { id }
}

pub(crate) fn packages() -> std::path::PathBuf {
    std::env::var_os("STEP0_PLUGINS")
        .map(Into::into)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins"))
}

pub(crate) fn summary(name: &str, values: &mut [std::time::Duration]) {
    values.sort_unstable();
    let at = |fraction: usize| {
        values[(values.len() * fraction).div_ceil(100) - 1].as_secs_f64() * 1_000.0
    };
    eprintln!(
        "{name} n={} p50={:.3}ms p95={:.3}ms max={:.3}ms",
        values.len(),
        at(50),
        at(95),
        at(100)
    );
}
