//! Lease-bound local sample images for the renderer fixture.
use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::hmac;

const IMAGE_FIELD: &[u8] = b"fixture-row-image:notes:thumbnail:";

impl Host {
    pub(super) fn row_image_ref(&self, lease: Uuid, row: Uuid, revision: Uuid) -> String {
        let mut token = [0_u8; 80];
        token[..16].copy_from_slice(lease.as_bytes());
        token[16..32].copy_from_slice(row.as_bytes());
        token[32..48].copy_from_slice(revision.as_bytes());
        let signature = hmac::sign(&self.undo_key, &[IMAGE_FIELD, &token[..48]].concat());
        token[48..].copy_from_slice(signature.as_ref());
        URL_SAFE_NO_PAD.encode(token)
    }

    async fn authorize_image(&self, headers: &HeaderMap, token: &str) -> Reply<()> {
        let (actor, session) = self.session(headers)?;
        if token.len() != 107 {
            return Err(HIDDEN);
        }
        let token: [u8; 80] = URL_SAFE_NO_PAD
            .decode(token)
            .map_err(|_| HIDDEN)?
            .try_into()
            .map_err(|_| HIDDEN)?;
        hmac::verify(
            &self.undo_key,
            &[IMAGE_FIELD, &token[..48]].concat(),
            &token[48..],
        )
        .map_err(|_| HIDDEN)?;
        let lease = Uuid::from_slice(&token[..16]).map_err(|_| HIDDEN)?;
        let row = Uuid::from_slice(&token[16..32]).map_err(|_| HIDDEN)?;
        let revision = Uuid::from_slice(&token[32..48]).map_err(|_| HIDDEN)?;
        if headers
            .get("x-step0-view-lease")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok())
            != Some(lease)
        {
            return Err(HIDDEN);
        }
        let (_, view) = self.lease(actor, session, lease).await?;
        let _fence = view.gate.lock().await;
        let (lease, _) = self.lease(actor, session, lease).await?;
        if lease.audience == "guest"
            || view
                .display
                .as_ref()
                .is_none_or(|display| !display.has_image(row, revision))
        {
            return Err(HIDDEN);
        }
        Ok(())
    }
}

pub(super) async fn read(
    State(host): State<Arc<Host>>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(error) = host.authorize_image(&headers, &token).await {
        return reply(Err(error));
    }
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        include_bytes!("sample-thumbnail.png").as_slice(),
    )
        .into_response()
}
