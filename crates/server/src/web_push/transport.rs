//! VAPID signing, Web Push encryption, and guarded HTTPS delivery.

use super::db::Subscription;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use calternal_core::PushPayload;
use calternal_types::Error;
use openssl::{
    bn::BigNumContext,
    ec::{EcGroup, EcKey, EcPoint, PointConversionForm},
    nid::Nid,
};
use reqwest::{Client, StatusCode, Url};
use std::{io::ErrorKind, path::Path, time::Duration};
use web_push::{ContentEncoding, SubscriptionInfo, VapidSignatureBuilder, WebPushMessageBuilder};
use zeroize::Zeroizing;

const KEY_FILE: &str = "web-push-vapid-v1.der";
const TTL: u32 = 300;

pub(super) struct Transport {
    http: Client,
    private: Zeroizing<String>,
    pub(super) public: String,
    contact: String,
}

fn unavailable(_: impl std::fmt::Display) -> Error {
    Error::unavailable("Web push key or transport is unavailable.")
}

impl Transport {
    pub(super) fn open(appdata: &Path, contact: String) -> Result<Self, Error> {
        let path = appdata.join("system").join(KEY_FILE);
        let data = match crate::asset_cache::read(&path, 512).map_err(unavailable)? {
            Some(data) => Zeroizing::new(data),
            None => match path.symlink_metadata() {
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    let group =
                        EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).map_err(unavailable)?;
                    let key = EcKey::generate(&group).map_err(unavailable)?;
                    let der = Zeroizing::new(key.private_key_to_der().map_err(unavailable)?);
                    crate::asset_cache::write(&path, &der).map_err(unavailable)?;
                    der
                }
                _ => return Err(unavailable("invalid stored key")),
            },
        };
        let key = EcKey::private_key_from_der(&data).map_err(unavailable)?;
        if key.group().curve_name() != Some(Nid::X9_62_PRIME256V1) {
            return Err(unavailable("invalid curve"));
        }
        key.check_key().map_err(unavailable)?;
        let mut ctx = BigNumContext::new().map_err(unavailable)?;
        let private = Zeroizing::new(key.private_key().to_vec_padded(32).map_err(unavailable)?);
        let public = key
            .public_key()
            .to_bytes(key.group(), PointConversionForm::UNCOMPRESSED, &mut ctx)
            .map_err(unavailable)?;
        Ok(Self {
            http: crate::netguard::client_for(None, Duration::from_secs(8)).map_err(unavailable)?,
            private: Zeroizing::new(B64.encode(&*private)),
            public: B64.encode(public),
            contact,
        })
    }

    pub(super) fn message(
        &self,
        sub: &Subscription,
        payload: &PushPayload,
    ) -> Result<reqwest::RequestBuilder, Error> {
        let endpoint = endpoint(&sub.endpoint)?;
        let info = SubscriptionInfo::new(&sub.endpoint, &sub.p256dh, &sub.auth);
        let mut vapid =
            VapidSignatureBuilder::from_base64(&self.private, &info).map_err(unavailable)?;
        vapid.add_claim("sub", self.contact.clone());
        let data = serde_json::to_vec(payload).map_err(unavailable)?;
        let mut message = WebPushMessageBuilder::new(&info);
        message.set_ttl(TTL);
        message.set_vapid_signature(vapid.build().map_err(unavailable)?);
        message.set_payload(ContentEncoding::Aes128Gcm, &data);
        let message = message.build().map_err(unavailable)?;
        let encrypted = message
            .payload
            .ok_or_else(|| unavailable("missing encrypted payload"))?;
        let mut request = self
            .http
            .post(endpoint)
            .header("TTL", TTL)
            .header("Content-Encoding", "aes128gcm")
            .header("Content-Type", "application/octet-stream");
        for (name, value) in encrypted.crypto_headers {
            let mut value = reqwest::header::HeaderValue::from_str(&value).map_err(unavailable)?;
            value.set_sensitive(true);
            request = request.header(name, value);
        }
        Ok(request.body(encrypted.content))
    }
}

pub(super) fn gone(status: StatusCode) -> bool {
    matches!(status, StatusCode::NOT_FOUND | StatusCode::GONE)
}

fn endpoint(text: &str) -> Result<Url, Error> {
    let invalid = || Error::invalid_input("Use a supported browser push subscription.");
    if text.len() > 2048 || text.bytes().any(|b| b.is_ascii_control()) {
        return Err(invalid());
    }
    let parsed = Url::parse(text).map_err(|_| invalid())?;
    let host = parsed.domain().ok_or_else(invalid)?;
    if !matches!(
        host,
        "fcm.googleapis.com" | "updates.push.services.mozilla.com"
    ) && !host.ends_with(".push.apple.com")
        && !host.ends_with(".notify.windows.com")
    {
        return Err(invalid());
    }
    let checked = crate::netguard::checked_url(text, &[host]).ok_or_else(invalid)?;
    if checked.as_str() != text {
        return Err(invalid());
    }
    Ok(checked)
}

pub(super) fn validate(endpoint_url: &str, p256dh: &str, auth: &str) -> Result<(), Error> {
    endpoint(endpoint_url)?;
    let invalid = || Error::invalid_input("Invalid browser push encryption keys.");
    if p256dh.len() != 87 || auth.len() != 22 {
        return Err(invalid());
    }
    let key = B64.decode(p256dh).map_err(|_| invalid())?;
    if key.len() != 65 || key[0] != 4 || B64.decode(auth).map_err(|_| invalid())?.len() != 16 {
        return Err(invalid());
    }
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).map_err(unavailable)?;
    let mut ctx = BigNumContext::new().map_err(unavailable)?;
    let point = EcPoint::from_bytes(&group, &key, &mut ctx).map_err(|_| invalid())?;
    if !point.is_on_curve(&group, &mut ctx).map_err(|_| invalid())? {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_core::PushStatus;

    #[test]
    fn restricted_endpoints_fail_closed() {
        for url in [
            "https://fcm.googleapis.com.evil.invalid/a",
            "https://example.com/a",
            "https://127.0.0.1/a",
            "https://user@fcm.googleapis.com/a",
            "https://fcm.googleapis.com:8443/a",
            "https://fcm.googleapis.com/a#fragment",
            "http://fcm.googleapis.com/a",
        ] {
            assert!(endpoint(url).is_err());
        }
        assert!(endpoint("https://fcm.googleapis.com/a").is_ok());
        assert!(endpoint("https://web.push.apple.com/a").is_ok());
    }

    #[test]
    fn key_is_stable_and_payload_is_encrypted() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("system")).unwrap();
        let open = || Transport::open(root.path(), "https://example.com".into()).unwrap();
        let transport = open();
        assert_eq!(transport.public, open().public);
        let file = root.path().join("system").join(KEY_FILE);
        assert_eq!(
            std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let device = Subscription {
            device: calternal_types::InstallationId::parse("01932910-0000-7000-8000-000000000001")
                .unwrap(),
            generation: String::new(),
            endpoint: "https://fcm.googleapis.com/test".into(),
            p256dh: transport.public.clone(),
            auth: B64.encode([7; 16]),
        };
        assert!(validate(&device.endpoint, &device.p256dh, &device.auth).is_ok());
        let payload = PushPayload {
            title: "Fixture thread title".into(),
            status: PushStatus::NeedsYou,
        };
        assert_eq!(
            serde_json::to_value(&payload).unwrap(),
            serde_json::json!({"title":"Fixture thread title","status":"needs_you"})
        );
        let req = transport
            .message(&device, &payload)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(req.headers()["content-encoding"], "aes128gcm");
        assert!(req.headers()["authorization"].is_sensitive());
        let body = req.body().unwrap().as_bytes().unwrap();
        assert!(
            !body
                .windows(payload.title.len())
                .any(|part| part == payload.title.as_bytes())
        );
    }
}
