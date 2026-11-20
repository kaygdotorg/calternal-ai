//! Product-path proof of the launcher installed by the guest image.

use super::*;
use crate::sandbox::worker;

const PACKAGE: &str = r#"
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
await Promise.resolve();
const assert = require('node:assert/strict');
const os = require('node:os');
assert.deepEqual(os.cpus(), []);
assert.ok(os.availableParallelism() > 0);
const crypto = require('node:crypto');
const tls = require('node:tls');
const vm = require('node:vm');
const { Duplex } = require('node:stream');
const digest = value => crypto.createHash('sha256').update(value).digest('hex');
const launcher = process.argv[1].endsWith('/standby.cjs');
if (launcher) {
    const copy = '/opt/calternal-standby/reseed-second.node';
    const denied = { code: 'ERR_ACCESS_DENIED' };
    assert.equal(process.permission.has('addon'), false);
    assert.equal(process.mainModule, undefined);
    assert.equal(require.main, undefined);
    const trusted = require.cache['/opt/calternal-standby/standby.cjs'];
    assert.ok(trusted);
    assert.ok(trusted.children.every(child => !child.filename.endsWith('.node')));
    assert.ok(Object.keys(require.cache).every(path => !path.endsWith('/reseed.node')));
    assert.throws(() => require(copy), denied);
    assert.throws(() => process.dlopen({ exports: {} }, copy), denied);
    assert.throws(() => new (require('node:worker_threads').Worker)('0', { eval: true }), denied);
    assert.throws(() => require('node:child_process').spawnSync('/bin/true'), denied);
    assert.throws(() => require('node:fs').writeFileSync('/tmp/calternal-denied', 'x'), denied);
    assert.throws(() => process.binding('natives'), denied);
}
const socket = new Duplex({
    read() {},
    write(chunk, encoding, done) {
        assert.equal(chunk[0], 22);
        assert.equal(chunk[5], 1);
        assert.ok(chunk.length >= 43);
        console.log(JSON.stringify({
            launcher,
            rss: process.memoryUsage.rss(),
            bytes: digest(crypto.randomBytes(32)),
            uuid: digest(crypto.randomUUID()),
            values: digest(crypto.getRandomValues(new Uint8Array(32))),
            key: digest(crypto.generateKeyPairSync('ed25519').publicKey.export({type:'spki', format:'der'})),
            hello: digest(chunk),
            helloRandom: digest(chunk.subarray(11,43)),
            math: digest(Array.from({length: 4}, () => Math.random()).join()),
            context: digest(vm.runInNewContext('Math.random()').toString()),
        }));
        done();
        setImmediate(() => {
            client.destroy();
            socket.destroy();
        });
    },
});
const client = tls.connect({socket, servername:'fixture.invalid'});
"#;

const BINARY_PACKAGE: &str = r#"
const expected = Buffer.from('000d0afffefd0d780a800171', 'hex');
let received = Buffer.alloc(0);
process.stdin.on('data', chunk => {
    received = Buffer.concat([received, chunk]);
    if (received.length < expected.length) return;
    if (!received.equals(expected)) process.exit(3);
    process.stdout.write(JSON.stringify({
        launcher: process.argv[1].endsWith('/standby.cjs'), bytes: received.toString('hex'),
    }), () => process.exit(0));
});
"#;

const HEAP_PACKAGE: &str = r#"
console.log(process.argv[1].endsWith('/standby.cjs') ? 'standby' : 'fresh');
const retained = [];
for (let i = 0; i < 2048; i++) retained.push(Array(16384).fill(i));
process.exit(23);
"#;

async fn draw(worker: &Placement) -> serde_json::Value {
    response(worker, &[]).await
}

async fn response(worker: &Placement, frames: &[&[u8]]) -> serde_json::Value {
    let exec = worker
        .exec(ExecRequest {
            argv: worker::ARGV.map(str::to_owned).to_vec(),
            interactive: true,
            ..Default::default()
        })
        .await
        .unwrap();
    for frame in frames {
        exec.input
            .as_ref()
            .unwrap()
            .send(frame.to_vec())
            .await
            .unwrap();
    }
    let (out, err, status) =
        tokio::time::timeout(Duration::from_secs(12), crate::sandbox::collect(exec))
            .await
            .unwrap();
    assert_eq!(status.code, Some(0), "{}", String::from_utf8_lossy(&err));
    serde_json::from_slice(&out).unwrap()
}

#[tokio::test]
async fn image_standby_refreshes_randomness_and_preserves_denials() {
    let Some(config) = std::env::var_os("CALTERNAL_SMALL_VM_TEST_CONFIG").map(PathBuf::from) else {
        assert!(std::env::var_os("CALTERNAL_SMALL_VM_PROOF_REQUIRED").is_none());
        eprintln!("SKIP live KVM fixture: CALTERNAL_SMALL_VM_TEST_CONFIG is unset");
        return;
    };
    let root = tempfile::Builder::new()
        .prefix("sv")
        .rand_bytes(5)
        .tempdir_in(config.parent().unwrap())
        .unwrap();
    let mut oversized: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
    oversized["idle_seconds"] = serde_json::json!(u32::MAX);
    let invalid = root.path().join("oversized.json");
    std::fs::write(&invalid, oversized.to_string()).unwrap();
    assert!(
        SmallRuntime::load(root.path().to_owned(), &invalid)
            .await
            .is_err()
    );
    let runtime = Arc::new(
        SmallRuntime::load(root.path().to_owned(), &config)
            .await
            .unwrap(),
    );
    runtime.initialize().await.unwrap();
    let space = SpaceId::new();
    let mut spec = SandboxSpec {
        account_id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        space_id: Some(space),
        image: Some(worker::SPACE_VM_IMAGE.into()),
        worker: Some(PluginWorker {
            space_id: space,
            source: PACKAGE.into(),
        }),
        limits: Limits::default(),
        code_directory: None,
        provider_login: false,
        shared_provider_sponsor: None,
        shared_tools: None,
    };
    let cold = runtime.start(&spec, None).await.unwrap();
    let fresh_draw = draw(&cold).await;
    assert_eq!(fresh_draw["launcher"], false);
    println!("Fresh plugin process RSS {} bytes", fresh_draw["rss"]);
    assert!(cold.checkpoint(1).await.unwrap());
    let saved = Generation::current(&cold.saved, &cold.binding)
        .unwrap()
        .unwrap();
    let mut observations = Vec::new();
    for _ in 0..2 {
        spec.thread_id = Uuid::new_v4();
        let restored = runtime.start(&spec, Some(1)).await.unwrap();
        assert_eq!(
            restored.slot.lock().await.vm.as_ref().unwrap().restored,
            Some(saved.id)
        );
        let draw = draw(&restored).await;
        assert_eq!(
            draw["launcher"], true,
            "product fell back instead of using the proven launcher"
        );
        println!("Restored plugin process RSS {} bytes", draw["rss"]);
        restored.stop().await.unwrap();
        observations.push(draw);
    }
    for field in [
        "bytes",
        "uuid",
        "values",
        "key",
        "hello",
        "helloRandom",
        "math",
        "context",
    ] {
        for draw in &observations {
            assert_eq!(draw[field].as_str().unwrap().len(), 64, "{field}");
        }
        assert_ne!(observations[0][field], observations[1][field], "{field}");
    }
    let frame = [0, 13, 10, 255, 254, 253, 13, b'x', 10, 128, 1, b'q'];
    let frames = [&frame[..2], &frame[2..8], &frame[8..]];
    spec.worker.as_mut().unwrap().source = BINARY_PACKAGE.into();
    spec.thread_id = Uuid::new_v4();
    let restored = runtime.start(&spec, Some(1)).await.unwrap();
    let standby = response(&restored, &frames).await;
    assert_eq!(standby["launcher"], true);
    restored.stop().await.unwrap();
    spec.thread_id = Uuid::new_v4();
    let fresh = runtime.start(&spec, None).await.unwrap();
    let fresh_bytes = response(&fresh, &frames).await;
    assert_eq!(fresh_bytes["launcher"], false);
    assert_eq!(fresh_bytes["bytes"], standby["bytes"]);
    fresh.stop().await.unwrap();

    // Freeze the adopted process before exec. Its task inventory remains
    // valid, but the trusted launcher can no longer acknowledge refresh.
    // The restored guest must reap it and execute these same bytes fresh.
    spec.thread_id = Uuid::new_v4();
    let failed_refresh = runtime.start(&spec, Some(1)).await.unwrap();
    assert_eq!(
        failed_refresh
            .slot
            .lock()
            .await
            .vm
            .as_ref()
            .unwrap()
            .restored,
        Some(saved.id)
    );
    failed_refresh.freeze().await.unwrap();
    let fallback = response(&failed_refresh, &frames).await;
    assert_eq!(fallback["launcher"], false);
    assert_eq!(fallback["bytes"], fresh_bytes["bytes"]);
    failed_refresh.stop().await.unwrap();

    spec.worker.as_mut().unwrap().source = HEAP_PACKAGE.into();
    for checkpoint in [None, Some(1)] {
        spec.thread_id = Uuid::new_v4();
        let placement = runtime.start(&spec, checkpoint).await.unwrap();
        let exec = placement
            .exec(ExecRequest {
                argv: worker::ARGV.map(str::to_owned).to_vec(),
                interactive: true,
                ..Default::default()
            })
            .await
            .unwrap();
        let (out, err, status) =
            tokio::time::timeout(Duration::from_secs(20), crate::sandbox::collect(exec))
                .await
                .unwrap();
        let error = String::from_utf8_lossy(&err);
        assert_eq!(
            out,
            if checkpoint.is_some() {
                b"standby\n".as_slice()
            } else {
                b"fresh\n"
            },
            "heap proof used the wrong launch mode"
        );
        assert!(matches!(status.code, Some(134 | 139)), "{status:?} {error}");
        assert!(error.contains("JavaScript heap out of memory"), "{error}");
        placement.stop().await.unwrap();
    }
    assert_eq!(
        Generation::current(&cold.saved, &cold.binding)
            .unwrap()
            .unwrap()
            .id,
        saved.id
    );
}
