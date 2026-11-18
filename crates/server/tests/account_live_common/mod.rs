//! Real host startup and loopback test services with throwaway keys.
#![allow(dead_code)]
use crate::common::{Device, Res};
use crate::machines::{BIN, Proc, installation};
use reqwest::{Client, Method, header};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use tempfile::TempDir;

pub struct Host {
    _child: Proc,
    dir: TempDir,
    pub origin: String,
    pub client: Client,
}

impl Host {
    pub fn start(configure: impl FnOnce(&mut Command)) -> Self {
        let dir = TempDir::new().unwrap();
        let mut cmd = Command::new(BIN);
        cmd.args([
            "serve",
            "--create",
            "--port",
            "0",
            "--plugin-root",
            "/nonexistent-test-plugins",
        ])
        .args(installation(dir.path()))
        .env_remove("CALTERNAL_ACCOUNT_EMAIL_REQUIRED")
        .env_remove("CALTERNAL_SMTP_HOST")
        .env_remove("CALTERNAL_FAULT")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null());
        configure(&mut cmd);
        let mut child = Proc(cmd.spawn().unwrap());
        let startup = wait_json(&mut child, &dir.path().join("appdata/startup.json"));
        let origin = startup["base_url"].as_str().unwrap().to_owned();
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap();
        Self {
            _child: child,
            dir,
            origin,
            client,
        }
    }

    pub fn appdata(&self) -> PathBuf {
        self.dir.path().join("appdata")
    }

    pub async fn get(&self, path: &str, cookie: Option<&str>) -> Res {
        self.request(Method::GET, path, cookie, None).await
    }

    pub async fn post(&self, path: &str, cookie: Option<&str>, input: Value) -> Res {
        self.request(Method::POST, path, cookie, Some(input)).await
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        cookie: Option<&str>,
        input: Option<Value>,
    ) -> Res {
        let mut request = self
            .client
            .request(method, format!("{}/api/v1/auth/{path}", self.origin))
            .header(header::ORIGIN, &self.origin);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        if let Some(input) = input {
            request = request.json(&input);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .map(|v| v.to_str().unwrap().to_owned());
        let body = response.json().await.unwrap_or(Value::Null);
        Res {
            status,
            cookie,
            body,
        }
    }

    pub async fn owner(&self) -> (String, String, Vec<String>, Device) {
        let token = std::fs::read_to_string(self.appdata().join("setup.token")).unwrap();
        let mut device = Device::new();
        let start = self
            .post(
                "setup/start",
                None,
                json!({
                    "token":token, "name":"Owner", "username":"owner-test",
                }),
            )
            .await;
        assert_eq!(start.status, 200, "{}", start.body);
        let created = self.post("setup/finish", None, json!({
            "ceremony":start.body["ceremony"], "credential":device.create_at(&self.origin, &start),
        })).await;
        assert_eq!(created.status, 200, "{}", created.body);
        let setup = created.session();
        let start = self.post("reverify/start", Some(&setup), json!({})).await;
        assert_eq!(start.status, 200, "{}", start.body);
        let fresh = self.post("reverify/finish", Some(&setup), json!({
            "ceremony":start.body["ceremony"], "credential":device.sign_at(&self.origin, &start),
        })).await;
        assert_eq!(fresh.status, 200, "{}", fresh.body);
        let codes = serde_json::from_value(created.body["recovery_codes"].clone()).unwrap();
        (setup, fresh.session(), codes, device)
    }
}

fn wait_json(child: &mut Proc, path: &Path) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(bytes) = std::fs::read(path)
            && let Ok(value) = serde_json::from_slice(&bytes)
        {
            return value;
        }
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "child exited before {}",
            path.display()
        );
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
