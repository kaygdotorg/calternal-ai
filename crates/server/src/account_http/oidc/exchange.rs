//! Discovery, outbound boundary checks and ID-token verification.

use super::{HttpAuth, Intent, PATH, Pending, Verified, failed};
use axum::response::Response;
use calternal_auth::OidcProvider;
use openidconnect::{
    AccessTokenHash, AuthType, AuthorizationCode, ClientId, ClientSecret, CsrfToken, HttpRequest,
    HttpResponse, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope,
    core::{
        CoreAuthPrompt, CoreAuthenticationFlow, CoreClient, CoreClientAuthMethod,
        CoreProviderMetadata,
    },
};
use reqwest::Url;
use std::io;

fn trusted(url: &Url, issuer: &Url, preset: &str) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && (url.origin() == issuer.origin()
            || (preset == "google"
                && url.port_or_known_default() == Some(443)
                && matches!(
                    url.host_str(),
                    Some("oauth2.googleapis.com" | "www.googleapis.com")
                )))
}

fn issuer(provider: &OidcProvider) -> Result<Url, Response> {
    Url::parse(&provider.issuer).map_err(|_| failed())
}

async fn request(
    http: reqwest::Client,
    issuer: Url,
    preset: String,
    request: HttpRequest,
) -> io::Result<HttpResponse> {
    let rejected = || io::Error::other("provider endpoint refused");
    let url = Url::parse(&request.uri().to_string()).map_err(|_| rejected())?;
    if !trusted(&url, &issuer, &preset) {
        return Err(rejected());
    }
    let (parts, body) = request.into_parts();
    let mut response = http
        .request(parts.method, url.clone())
        .headers(parts.headers)
        .body(body)
        .send()
        .await
        .map_err(|_| rejected())?;
    let mut builder = axum::http::Response::builder().status(response.status());
    if let Some(headers) = builder.headers_mut() {
        *headers = response.headers().clone();
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| rejected())? {
        if bytes.len() + chunk.len() > 1024 * 1024 {
            return Err(rejected());
        }
        bytes.extend_from_slice(&chunk);
    }
    if url.path().ends_with("/.well-known/openid-configuration") && preset != "apple" {
        let discovery: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| rejected())?;
        if !discovery["code_challenge_methods_supported"]
            .as_array()
            .is_some_and(|items| items.iter().any(|m| m == "S256"))
        {
            return Err(rejected());
        }
    }
    builder.body(bytes).map_err(|_| rejected())
}

fn redirect(s: &HttpAuth) -> Result<RedirectUrl, Response> {
    let url = s.config.origin.join(PATH).map_err(|_| failed())?;
    RedirectUrl::new(url.to_string()).map_err(|_| failed())
}

pub(super) async fn begin(
    s: &HttpAuth,
    intent: Intent,
) -> Result<(String, Url, Pending), Response> {
    let provider = &intent.provider;
    let issuer = issuer(provider)?;
    let outbound = |r| {
        request(
            s.oidc.client.clone(),
            issuer.clone(),
            provider.preset.clone(),
            r,
        )
    };
    let metadata = CoreProviderMetadata::discover_async(
        IssuerUrl::new(provider.issuer.clone()).map_err(|_| failed())?,
        &outbound,
    )
    .await
    .map_err(|_| failed())?;
    if !trusted(
        metadata.authorization_endpoint().url(),
        &issuer,
        &provider.preset,
    ) || !metadata
        .token_endpoint()
        .is_some_and(|e| trusted(e.url(), &issuer, &provider.preset))
    {
        return Err(failed());
    }

    let client = CoreClient::from_provider_metadata(
        metadata.clone(),
        ClientId::new(provider.client_id.clone()),
        None,
    )
    .set_redirect_uri(redirect(s)?);
    let mut authorization = client.authorize_url(
        CoreAuthenticationFlow::AuthorizationCode,
        CsrfToken::new_random,
        Nonce::new_random,
    );
    for scope in provider.scopes.iter().filter(|s| s.as_str() != "openid") {
        authorization = authorization.add_scope(Scope::new(scope.clone()));
    }
    let verifier = if provider.preset == "apple" {
        // Apple's discovery publishes code + confidential client authentication
        // and has no S256 capability. State, nonce and browser binding still apply.
        authorization = authorization.add_extra_param("response_mode", "form_post");
        None
    } else {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        authorization = authorization.set_pkce_challenge(challenge);
        Some(verifier)
    };
    if intent.reverify.is_some() {
        authorization = match provider.preset.as_str() {
            "apple" => authorization,
            "google" => authorization.add_prompt(CoreAuthPrompt::SelectAccount),
            _ => authorization
                .add_prompt(CoreAuthPrompt::Login)
                .set_max_age(std::time::Duration::ZERO),
        };
    }
    let (url, csrf, nonce) = authorization.url();
    Ok((
        csrf.secret().to_owned(),
        url,
        Pending {
            intent,
            metadata,
            nonce,
            verifier,
        },
    ))
}

pub(super) async fn check(
    s: &HttpAuth,
    intent: &Intent,
    metadata: CoreProviderMetadata,
    nonce: Nonce,
    verifier: Option<PkceCodeVerifier>,
    code: String,
) -> Result<Verified, Response> {
    let provider = &intent.provider;
    let issuer = issuer(provider)?;
    let outbound = |r| {
        request(
            s.oidc.client.clone(),
            issuer.clone(),
            provider.preset.clone(),
            r,
        )
    };
    let credential = s
        .auth
        .oidc_credential(provider, &intent.revision)
        .await
        .map_err(|_| failed())?;
    let credential = std::str::from_utf8(credential.expose()).map_err(|_| failed())?;
    let auth_type = if metadata
        .token_endpoint_auth_methods_supported()
        .is_some_and(|methods| {
            methods.contains(&CoreClientAuthMethod::ClientSecretPost)
                && !methods.contains(&CoreClientAuthMethod::ClientSecretBasic)
        }) {
        AuthType::RequestBody
    } else {
        AuthType::BasicAuth
    };
    let client = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(provider.client_id.clone()),
        Some(ClientSecret::new(credential.to_owned())),
    )
    .set_redirect_uri(redirect(s)?)
    .set_auth_type(auth_type);
    let mut exchange = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|_| failed())?;
    if let Some(verifier) = verifier {
        exchange = exchange.set_pkce_verifier(verifier);
    }
    let response = exchange
        .request_async(&outbound)
        .await
        .map_err(|_| failed())?;
    let id_token = response.extra_fields().id_token().ok_or_else(failed)?;
    let verifier = client.id_token_verifier();
    let claims = id_token.claims(&verifier, &nonce).map_err(|_| failed())?;
    if let Some(expected) = claims.access_token_hash() {
        let algorithm = id_token.signing_alg().map_err(|_| failed())?;
        let key = id_token.signing_key(&verifier).map_err(|_| failed())?;
        let actual = AccessTokenHash::from_token(response.access_token(), algorithm, key)
            .map_err(|_| failed())?;
        if &actual != expected {
            return Err(failed());
        }
    }
    let now = calternal_types::now_ms() / 1000;
    let issued = claims.issue_time().timestamp();
    if issued > now + 30 || issued < now - 600 {
        return Err(failed());
    }
    if intent.reverify.is_some()
        && provider.preset == "generic"
        && !claims
            .auth_time()
            .is_some_and(|t| t.timestamp() >= now - 120 && t.timestamp() <= now + 30)
    {
        return Err(failed());
    }
    let display_name = provider.display_name_claim.as_ref().and_then(|field| {
        serde_json::to_value(claims)
            .ok()?
            .get(field)?
            .as_str()
            .map(str::to_owned)
    });
    Ok(Verified {
        subject: claims.subject().as_str().to_owned(),
        display_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_cannot_redirect_secrets_to_an_unapproved_endpoint() {
        let generic = Url::parse("https://auth.example.com/application/o/test/").unwrap();
        assert!(trusted(
            &Url::parse("https://auth.example.com/token").unwrap(),
            &generic,
            "generic"
        ));
        for address in [
            "http://auth.example.com/token",
            "https://attacker.example.com/token",
            "https://auth.example.com@attacker.example.com/token",
            "https://auth.example.com:444/token",
        ] {
            assert!(
                !trusted(&Url::parse(address).unwrap(), &generic, "generic"),
                "{address}"
            );
        }

        let google = Url::parse("https://accounts.google.com").unwrap();
        assert!(trusted(
            &Url::parse("https://oauth2.googleapis.com/token").unwrap(),
            &google,
            "google"
        ));
        assert!(!trusted(
            &Url::parse("https://oauth2.googleapis.com:444/token").unwrap(),
            &google,
            "google"
        ));
    }
}
