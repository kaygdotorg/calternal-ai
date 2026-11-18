//! Unsplash API requests and image fetches. Authorization is sent only to the
//! API host. All failures leaving this module use fixed caller-safe messages.

use super::catalog::Rendition;
use crate::api::assets;
use calternal_types::Error;
use reqwest::{Url, header};
use serde::Deserialize;
use tokio::sync::Mutex;

const API: &str = "api.unsplash.com";
const IMAGES: &str = "images.unsplash.com";
const HOSTS: &[&str] = &[API, IMAGES];
const REQUESTS_PER_HOUR: usize = 48;
const MAX_JSON: usize = 128 * 1024;

pub(super) struct Upstream {
    client: reqwest::Client,
    authorization: header::HeaderValue,
    requests: Mutex<assets::Budget<()>>,
    #[cfg(test)]
    pub stand_in: Option<Url>,
}

pub(super) struct Source {
    raw: Url,
    download: Url,
}

impl Upstream {
    pub(super) fn new(key: &str) -> Result<Self, Error> {
        let mut authorization = header::HeaderValue::from_str(&format!("Client-ID {key}"))
            .map_err(|_| Error::invalid_input("invalid Unsplash access key configuration"))?;
        authorization.set_sensitive(true);
        let client = crate::netguard::client(HOSTS)
            .map_err(|_| Error::internal("could not initialize wallpaper fetch"))?;
        Ok(Self {
            client,
            authorization,
            requests: Mutex::new(assets::Budget::new()),
            #[cfg(test)]
            stand_in: None,
        })
    }

    pub(super) async fn source(&self, id: &str) -> Result<Source, Error> {
        #[derive(Deserialize)]
        struct Photo {
            id: String,
            urls: ImageUrls,
            links: PhotoLinks,
        }
        #[derive(Deserialize)]
        struct ImageUrls {
            raw: String,
        }
        #[derive(Deserialize)]
        struct PhotoLinks {
            download_location: String,
        }
        let url = Url::parse(&format!("https://{API}/photos/{id}")).expect("catalog photo ID");
        let photo: Photo = serde_json::from_slice(&self.get(url, MAX_JSON, false).await?)
            .map_err(|_| unavailable())?;
        let raw = checked_url(&photo.urls.raw, IMAGES)?;
        let download = checked_url(&photo.links.download_location, API)?;
        if photo.id != id || download.path() != format!("/photos/{id}/download") {
            return Err(unavailable());
        }
        Ok(Source { raw, download })
    }

    pub(super) async fn track(&self, source: &Source) -> Result<(), Error> {
        self.get(source.download.clone(), MAX_JSON, false).await?;
        Ok(())
    }

    pub(super) async fn image(&self, source: &Source, r: Rendition) -> Result<Vec<u8>, Error> {
        let mut url = source.raw.clone();
        // Keep the API's view-tracking parameter, applying the shell's
        // rendition values exactly once.
        let ixid = url
            .query_pairs()
            .find(|(k, _)| k == "ixid")
            .map(|(_, v)| v.into_owned());
        url.set_query(None);
        {
            let mut params = url.query_pairs_mut();
            if let Some(ixid) = ixid {
                params.append_pair("ixid", &ixid);
            }
            params
                .append_pair("w", &r.width.to_string())
                .append_pair("q", &r.quality.to_string())
                .append_pair("fit", "max")
                .append_pair("fm", "jpg");
            if let Some(blur) = r.blur {
                params.append_pair("blur", &blur.to_string());
            }
            if r.light {
                params
                    .append_pair("bri", "35")
                    .append_pair("con", "-25")
                    .append_pair("sat", "-8");
            }
        }
        let bytes = self.get(url, r.max_bytes, true).await?;
        if jpeg(&bytes) {
            Ok(bytes)
        } else {
            Err(unavailable())
        }
    }

    async fn get(&self, url: Url, cap: usize, jpeg: bool) -> Result<Vec<u8>, Error> {
        if !self
            .requests
            .lock()
            .await
            .admit((), REQUESTS_PER_HOUR, REQUESTS_PER_HOUR)
        {
            return Err(Error::unavailable("wallpaper fetch limit reached"));
        }
        let api = url.host_str() == Some(API);
        #[cfg(test)]
        let (url, client) = if let Some(origin) = &self.stand_in {
            let mut local = origin.clone();
            local.set_path(url.path());
            local.set_query(url.query());
            (
                local,
                reqwest::Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .expect("stand-in client"),
            )
        } else {
            (url, self.client.clone())
        };
        #[cfg(not(test))]
        let client = &self.client;
        let mut request = client.get(url);
        if api {
            request = request
                .header(header::AUTHORIZATION, self.authorization.clone())
                .header("Accept-Version", "v1");
        }
        assets::get(request, cap, jpeg.then_some("image/jpeg"), unavailable).await
    }
}

fn checked_url(text: &str, host: &str) -> Result<Url, Error> {
    assets::checked_url(text, &[host]).ok_or_else(unavailable)
}

/// JPEG marker check for trusted image-service output. We serve only a fixed
/// media type with nosniff and never decode the upstream image on the server.
pub(super) fn jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\xff\xd8\xff") && bytes.ends_with(b"\xff\xd9")
}

fn unavailable() -> Error {
    Error::not_found("Unsplash wallpaper is unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_types::ErrorCode;

    #[test]
    fn upstream_addresses_are_https_and_exact_host_only() {
        assert!(checked_url("https://images.unsplash.com/photo-x?ixid=ok", IMAGES).is_ok());
        for url in [
            "http://images.unsplash.com/photo-x",
            "https://images.unsplash.com:8443/photo-x",
            "https://images.unsplash.com.evil.invalid/photo-x",
            "https://127.0.0.1/photo-x",
            "https://api.unsplash.com/photo-x",
            "https://user@images.unsplash.com/photo-x",
            "https://images.unsplash.com./photo-x",
        ] {
            assert!(checked_url(url, IMAGES).is_err(), "{url}");
        }
    }

    #[tokio::test]
    async fn rolling_budget_charges_failures_and_recovers_after_an_hour() {
        let service = Upstream::new("fixture-only").unwrap();
        let url = Url::parse("https://unexpected.invalid/").unwrap();
        // The guarded DNS resolver refuses this undeclared name before using
        // system DNS. A failed request must still consume an outbound attempt.
        for _ in 0..REQUESTS_PER_HOUR {
            assert_eq!(
                service.get(url.clone(), 10, false).await.unwrap_err().code,
                ErrorCode::NotFound
            );
        }
        assert_eq!(
            service.get(url.clone(), 10, false).await.unwrap_err().code,
            ErrorCode::Unavailable
        );
        let later = std::time::Instant::now() + std::time::Duration::from_secs(3600);
        assert!(service.requests.lock().await.admit_at(
            later,
            (),
            REQUESTS_PER_HOUR,
            REQUESTS_PER_HOUR
        ));
    }
}
