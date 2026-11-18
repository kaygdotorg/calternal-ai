//! Private account onboarding and recovery requests. Never enter receipt replay.

use super::*;
use calternal_auth::AccountMail;

pub(super) fn routes(router: Router<HttpAuth>) -> Router<HttpAuth> {
    router
        .route("/api/v1/auth/email/start", post(email_start))
        .route("/api/v1/auth/email/confirm", post(email_confirm))
        .route("/api/v1/auth/username/change", post(username_change))
        .route("/api/v1/auth/recovery/codes/regenerate", post(codes))
        .route("/api/v1/auth/passkeys/prompt/dismiss", post(dismiss))
        .route("/api/v1/auth/recovery/start", post(start))
        .route("/api/v1/auth/recovery/prove", post(prove))
        .route("/api/v1/auth/recovery/status", post(status))
        .route("/api/v1/auth/recovery/veto", post(veto))
        .route("/api/v1/auth/recovery/enroll/start", post(enroll_start))
        .route("/api/v1/auth/recovery/enroll/finish", post(enroll_finish))
        .route("/api/v1/auth/recovery/history", get(history))
}

pub(super) fn private(payload: Value) -> Response {
    ([(CACHE_CONTROL, "no-store")], Json(payload)).into_response()
}

fn queue(s: &HttpAuth, mail: Option<AccountMail>) {
    if let (Some(sender), Some(delivery)) = (&s.mail, mail) {
        if !sender.notice(delivery) {
            tracing::warn!("account security mail queue unavailable");
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmailStart {
    email: String,
    token: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmailProof {
    challenge_id: String,
    code: String,
    token: Option<String>,
}

async fn email_start(State(s): State<HttpAuth>, req: Request) -> Reply {
    let source = source_key(&s, &req);
    let caller = identify(&s, req.headers()).await;
    let i: EmailStart = body(&s, req).await?;
    let caller = caller.filter(|_| i.token.is_none());
    if s.mail.is_none() && (i.token.is_some() || s.config.email_required) {
        return Err(failure(503, "email_delivery_failed", "mail unavailable"));
    }
    let (payload, delivery) = s
        .auth
        .start_account_email(
            caller,
            i.token.as_deref(),
            &i.email,
            &source,
            s.mail.is_some(),
        )
        .await
        .map_err(error)?;
    if let (Some(sender), Some(delivery)) = (&s.mail, delivery) {
        if !sender.challenge(delivery).await {
            return Err(failure(
                503,
                "email_delivery_failed",
                "mail delivery failed",
            ));
        }
    }
    Ok(private(payload))
}

async fn email_confirm(State(s): State<HttpAuth>, req: Request) -> Reply {
    let source = source_key(&s, &req);
    let caller = identify(&s, req.headers()).await;
    let i: EmailProof = body(&s, req).await?;
    if s.mail.is_none() {
        return Err(failure(503, "email_delivery_failed", "mail unavailable"));
    }
    let caller = caller.filter(|_| i.token.is_none());
    let (payload, mail) = s
        .auth
        .confirm_account_email(
            caller,
            i.token.as_deref(),
            &i.challenge_id,
            &i.code,
            &source,
        )
        .await
        .map_err(error)?;
    queue(&s, mail);
    Ok(private(payload))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Username {
    username: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

async fn username_change(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, i) = session(&s, req.headers()).await?;
    let request: Username = body(&s, req).await?;
    Ok(private(
        json!({"username":s.auth.change_username(a,i,&request.username).await.map_err(error)?}),
    ))
}

async fn codes(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, i) = session(&s, req.headers()).await?;
    let _: Empty = body(&s, req).await?;
    Ok(private(
        json!({"recovery_codes":s.auth.regenerate_recovery_codes(a,i).await.map_err(error)?}),
    ))
}

async fn dismiss(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, _) = session(&s, req.headers()).await?;
    let _: Empty = body(&s, req).await?;
    s.auth.dismiss_second_passkey(a).await.map_err(error)?;
    Ok(private(json!({"second_passkey_prompt":false})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryStart {
    method: String,
    email: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryProof {
    flow_id: String,
    code: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryStatus {
    flow_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Enrollment {
    enrollment_grant: String,
}

async fn start(State(s): State<HttpAuth>, req: Request) -> Reply {
    let source = source_key(&s, &req);
    let i: RecoveryStart = body(&s, req).await?;
    if i.method == "email" && s.mail.is_none() {
        return Err(failure(503, "email_delivery_failed", "mail unavailable"));
    }
    let (payload, mail) = s
        .auth
        .start_recovery(&i.method, i.email.as_deref(), &source, s.mail.is_some())
        .await
        .map_err(error)?;
    queue(&s, mail);
    Ok(private(payload))
}

async fn prove(State(s): State<HttpAuth>, req: Request) -> Reply {
    let source = source_key(&s, &req);
    let i: RecoveryProof = body(&s, req).await?;
    let (payload, mail) = s
        .auth
        .prove_recovery(&i.flow_id, &i.code, &source, s.mail.is_some())
        .await
        .map_err(error)?;
    queue(&s, mail);
    Ok(private(payload))
}

async fn status(State(s): State<HttpAuth>, req: Request) -> Reply {
    let source = source_key(&s, &req);
    let i: RecoveryStatus = body(&s, req).await?;
    let (payload, mail) = s
        .auth
        .recovery_status(&i.flow_id, &source, s.mail.is_some())
        .await
        .map_err(error)?;
    queue(&s, mail);
    Ok(private(payload))
}

async fn veto(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, i) = session(&s, req.headers()).await?;
    let _: Empty = body(&s, req).await?;
    Ok(private(
        json!({"cancelled":s.auth.veto_recovery(a,i).await.map_err(error)?}),
    ))
}

async fn enroll_start(State(s): State<HttpAuth>, req: Request) -> Reply {
    let source = source_key(&s, &req);
    let i: Enrollment = body(&s, req).await?;
    let (ceremony, options) = s
        .auth
        .start_replacement(&i.enrollment_grant, &source)
        .await
        .map_err(error)?;
    Ok(private(json!({"ceremony":ceremony,"options":options})))
}

async fn enroll_finish(State(s): State<HttpAuth>, req: Request) -> Reply {
    let i: Finish = body(&s, req).await?;
    let mail = s
        .auth
        .finish_replacement(&i.ceremony, i.credential)
        .await
        .map_err(error)?;
    queue(&s, mail);
    Ok(private(json!({"sign_in_required":true})))
}

async fn history(State(s): State<HttpAuth>, req: Request) -> Reply {
    let (a, _) = session(&s, req.headers()).await?;
    Ok(private(
        json!({"events":s.auth.recovery_history(a).await.map_err(error)?}),
    ))
}
