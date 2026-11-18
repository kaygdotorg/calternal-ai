//! Person-account erasure via the trusted origin and installation checks.
use super::*;
use crate::{api::Shared, auth::Directory};
use calternal_auth::erasure::AccountErasure;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    account_id: Option<AccountId>,
}

pub(crate) async fn delete<D: Directory>(
    State(state): State<Shared<D>>,
    request: Request,
) -> Reply {
    let service = state
        .directory
        .http_auth()
        .cloned()
        .ok_or_else(unauthorized)?;
    let (actor, installation) = session(&service, request.headers()).await?;
    let target = body::<Selection>(&service, request)
        .await?
        .account_id
        .unwrap_or(actor);
    // Disconnecting the HTTP caller does not cancel an admitted erasure.
    tokio::spawn(erase(state, service, actor, installation, target))
        .await
        .map_err(|_| {
            crate::api::error_response(calternal_types::Error::unavailable(
                "Account cleanup is pending.",
            ))
        })?
}

async fn erase<D: Directory>(
    state: Shared<D>,
    service: HttpAuth,
    actor: AccountId,
    installation: InstallationId,
    target: AccountId,
) -> Reply {
    let lifecycle = state.store.content().lifecycle();
    let outcome = service
        .auth
        .erase_account(actor, installation, target, move |owners| {
            for owner in owners {
                lifecycle.close(owner.uuid());
            }
            crate::fault::hit(crate::fault::FaultPoint::ErasureAuthorityCommitted);
        })
        .await
        .map_err(|failure| {
            if let Error::Refused {
                code: ErrorCode::Forbidden,
                detail,
            } = &failure
            {
                return classified_failure(403, "forbidden", detail, false);
            }
            if matches!(&failure, Error::Internal(_)) {
                tracing::warn!(error = %failure, "account authority erasure failed");
            }
            error(failure)
        })?;
    let owners = match outcome {
        AccountErasure::Committed(owners) => owners,
        AccountErasure::Transfer(spaces) => {
            let names = spaces
                .iter()
                .map(|space| format!("{} ({})", space.name, space.space_id))
                .collect::<Vec<_>>()
                .join(", ");
            let message = format!(
                "Transfer ownership of these shared spaces before deleting the account: {names}. Run calternal people transfer --space SPACE_UUID ACCOUNT_UUID."
            );
            return Err(crate::api::error_response(calternal_types::Error::conflict(message)
                .with_details(json!({"spaces":spaces,"transfer_command":"calternal people transfer --space SPACE_UUID ACCOUNT_UUID"}))));
        }
    };
    let spaces = service.auth.spaces();
    tokio::task::spawn_blocking(move || {
        crate::erasure::finish(&state.store, Some(&state.jobs), &spaces, owners)
    })
    .await
    .map_err(|_| {
        crate::api::error_response(calternal_types::Error::unavailable(
            "Account cleanup is pending.",
        ))
    })?
    .map_err(crate::api::error_response)?;
    let mut response = Json(json!({"account_id":target,"deleted":true})).into_response();
    if target == actor {
        response.headers_mut().insert(
            SET_COOKIE,
            HeaderValue::from_static(
                "__Host-calternal_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Secure",
            ),
        );
    }
    Ok(response)
}
