//! Perf-only measurements for the shipped backend and the Podman control.

use std::path::Path;
use std::time::{Duration, Instant};

use calternal_types::SpaceId;
use serde_json::json;
use uuid::Uuid;

use super::{Backend, Config, MicrovmBackend};
use crate::protocol::SandboxSpec;
use crate::sandbox::{ExecRequest, Sandbox, collect, podman::PodmanBackend};

fn kv(path: &str, name: &str) -> u64 {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix(name))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse::<u64>()
        .unwrap()
}

fn mem_available() -> u64 {
    kv("/proc/meminfo", "MemAvailable:")
}

fn spec() -> SandboxSpec {
    SandboxSpec {
        space_id: Some(SpaceId::new()),
        provider_login: false,
        account_id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        code_directory: None,
        image: None,
        worker: None,
        limits: Default::default(),
    }
}

async fn command(sandbox: &dyn Sandbox) {
    let req = ExecRequest {
        argv: vec!["true".into()],
        ..ExecRequest::default()
    };
    let exec = sandbox.exec(req).await.unwrap();
    let (_, err, status) = collect(exec).await;
    assert_eq!(
        status.code,
        Some(0),
        "{status:?} {}",
        String::from_utf8_lossy(&err)
    );
}

async fn starts(backend: &dyn Backend) {
    backend.initialize().await.unwrap();
    for _ in 0..10 {
        let spec = spec();
        let t = Instant::now();
        let first = backend.start(&spec).await.unwrap();
        command(&*first).await;
        let cold_ms = t.elapsed().as_secs_f64() * 1000.0;
        let second_spec = SandboxSpec {
            thread_id: Uuid::new_v4(),
            ..spec.clone()
        };
        let t = Instant::now();
        let second = backend.start(&second_spec).await.unwrap();
        command(&*second).await;
        let second_ms = t.elapsed().as_secs_f64() * 1000.0;
        let mut commands = Vec::new();
        for _ in 0..30 {
            let t = Instant::now();
            command(&*second).await;
            commands.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        println!(
            "{}",
            json!({"backend": backend.name(), "first_thread_ms": cold_ms, "second_thread_ms": second_ms, "commands_ms": commands})
        );
        second.stop().await.unwrap();
        first.stop().await.unwrap();
        backend.erase(spec.account_id, None).await.unwrap();
    }
    backend.shutdown().await.unwrap();
}

async fn podman_density(backend: &dyn Backend, capacity: usize, reserve_kib: u64) {
    backend.initialize().await.unwrap();
    let baseline = mem_available();
    let mut people = Vec::new();
    for _ in 0..capacity {
        if mem_available() < reserve_kib {
            break;
        }
        let spec = spec();
        let sandbox = backend.start(&spec).await.unwrap();
        command(&*sandbox).await;
        people.push((spec.account_id, sandbox));
    }
    tokio::time::sleep(Duration::from_secs(5)).await;
    let mut samples = Vec::new();
    for _ in 0..5 {
        samples.push(mem_available());
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let stop = if people.len() == capacity {
        "configured_cap"
    } else {
        "host_reserve"
    };
    println!(
        "{}",
        json!({"backend":"podman","idle_people":people.len(),"stop":stop,"host_baseline_kib":baseline,"host_available_kib":samples})
    );
    for (account, sandbox) in people {
        sandbox.stop().await.unwrap();
        backend.erase(account, None).await.unwrap();
    }
    backend.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "run the dev-built binary only under the perf lock with the pinned guest image"]
async fn compare_microvm_and_podman_under_perf_lock() {
    assert_eq!(std::env::var("CALTERNAL_PERF_HELD").as_deref(), Ok("1"));
    let host_kib = kv("/proc/meminfo", "MemTotal:");
    assert!(
        (7 * 1024 * 1024..=9 * 1024 * 1024).contains(&host_kib),
        "use an 8 GB perf VM"
    );
    let config = std::env::var_os("CALTERNAL_MICROVM_TEST_CONFIG").unwrap();
    let mut config = Config::load(Path::new(&config)).await.unwrap();
    let root = tempfile::tempdir().unwrap();
    // The TAP namespace can have only the invoking mapped UID. Measure the
    // host Podman control outside that namespace with the same perf lock.
    if config.network_policy.is_none() {
        let image = "localhost/calternal-standard:current";
        let podman = PodmanBackend::new("podman", root.path().join("podman"), image);
        starts(&podman).await;
        podman_density(
            &podman,
            config.max_guests,
            (1024 + u64::from(config.memory_mib)) * 1024,
        )
        .await;
    }
    let desired_pool = config.warm_guests;
    config.warm_guests = 0;
    let backend = MicrovmBackend::new(root.path().join("starts"), config.clone());
    starts(&backend).await;

    let backend = MicrovmBackend::new(root.path().join("density"), config.clone());
    backend.initialize().await.unwrap();
    let baseline = mem_available();
    let mut guests = Vec::new();
    let mut account_ids = Vec::new();
    // Leave room for the host and then one full guest reservation before
    // admitting the next idle account. Report the last observed population.
    for _ in 0..config.max_guests {
        if mem_available() < (1024 + u64::from(config.memory_mib)) * 1024 {
            break;
        }
        let spec = spec();
        let sandbox = backend.start(&spec).await.unwrap();
        command(&*sandbox).await;
        let slot = backend.0.account(spec.account_id);
        let pid = slot.lock().await.vm.as_ref().unwrap().pid();
        guests.push((pid, sandbox));
        account_ids.push(spec.account_id);
    }
    tokio::time::sleep(Duration::from_secs(5)).await;
    let mut samples = Vec::new();
    for _ in 0..5 {
        let pss = guests
            .iter()
            .map(|(pid, _)| kv(&format!("/proc/{pid}/smaps_rollup"), "Pss:"))
            .sum::<u64>();
        samples.push(json!({"vmm_pss_kib": pss, "host_available_kib": mem_available()}));
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let stop = if guests.len() == config.max_guests {
        "configured_cap"
    } else {
        "host_reserve"
    };
    println!(
        "{}",
        json!({"backend":"microvm","idle_people":guests.len(),"stop":stop,"host_baseline_kib":baseline,"samples":samples})
    );
    for ((_, sandbox), account) in guests.into_iter().zip(account_ids) {
        sandbox.stop().await.unwrap();
        backend.erase(account, None).await.unwrap();
    }
    backend.shutdown().await.unwrap();

    if desired_pool > 0 {
        config.warm_guests = desired_pool;
        let pool = MicrovmBackend::new(root.path().join("pool"), config);
        let time = Instant::now();
        pool.initialize().await.unwrap();
        tokio::time::timeout(Duration::from_secs(180), async {
            while pool.0.warm.lock().await.len() < desired_pool {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        println!(
            "{}",
            json!({"backend":"microvm","warm_guests":desired_pool,"preboot_ms":time.elapsed().as_secs_f64()*1000.0})
        );
        let spec = spec();
        let time = Instant::now();
        let picked_up = pool.start(&spec).await.unwrap();
        command(&*picked_up).await;
        let warm_pickup_ms = time.elapsed().as_secs_f64() * 1000.0;
        picked_up.release().await.unwrap();
        drop(picked_up);

        // Advance the idle timestamp while exercising the actual sweep path
        // and persisted disk. The benchmark times the wake, not the wait.
        let slot = pool.0.account(spec.account_id);
        slot.lock().await.last_used = Instant::now() - super::IDLE;
        drop(slot);
        tokio::time::timeout(Duration::from_secs(90), async {
            loop {
                if pool.0.account(spec.account_id).lock().await.vm.is_none() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap();
        let time = Instant::now();
        let restarted = pool.start(&spec).await.unwrap();
        command(&*restarted).await;
        let idle_wake_ms = time.elapsed().as_secs_f64() * 1000.0;
        println!(
            "{}",
            json!({
                "backend":"microvm",
                "warm_pickup_ms":warm_pickup_ms,
                "idle_wake_ms":idle_wake_ms
            })
        );
        restarted.release().await.unwrap();
        pool.erase(spec.account_id, None).await.unwrap();
        pool.shutdown().await.unwrap();
    }
}
