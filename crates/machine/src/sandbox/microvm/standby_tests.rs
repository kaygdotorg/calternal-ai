//! Product-path proof of the launcher installed by the guest image.

use super::*;
use crate::sandbox::worker;

const PACKAGE: &str = r#"
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
await Promise.resolve();
const assert = require('node:assert/strict');
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

async fn draw(worker: &Placement) -> serde_json::Value {
    let exec = worker
        .exec(ExecRequest {
            argv: worker::ARGV.map(str::to_owned).to_vec(),
            interactive: true,
            ..Default::default()
        })
        .await
        .unwrap();
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
    assert_eq!(draw(&cold).await["launcher"], false);
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
    assert_eq!(
        Generation::current(&cold.saved, &cold.binding)
            .unwrap()
            .unwrap()
            .id,
        saved.id
    );
}
