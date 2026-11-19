//! Host-only provider configuration and verified-identity binding.

use crate::db::{now_ms, sql};
use crate::{
    AccountId, Auth, Error, ErrorCode, InstallationId, Result, account_details, account_recovery,
    session, spaces,
};
use calternal_secrets::SecretValue;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Deployment-controlled OIDC settings. Secret material travels separately.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OidcProvider {
    /// Stable local provider key.
    pub id: String,
    /// Host-owned presentation preset: apple, google or generic.
    pub preset: String,
    /// Button label rendered by the host.
    pub label: String,
    /// Exact HTTPS issuer validated in discovery and ID tokens.
    pub issuer: String,
    /// Public OAuth client ID.
    pub client_id: String,
    /// Additional requested OIDC scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Optional standard ID-token profile claim used as a name suggestion.
    pub display_name_claim: Option<String>,
}

/// Provider and external identity bound to one browser installation.
pub struct OidcSession {
    /// Local provider configuration ID.
    pub provider: String,
    /// Exact issuer-scoped OIDC subject.
    pub subject: String,
}

/// Invitation-bound profile for a provider-first signup.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcSignup {
    /// Single-use invitation from the host.
    pub token: String,
    /// Optional display name typed by the user. The provider can suggest it.
    pub name: Option<String>,
    /// Requested lowercase presentation handle.
    pub username: String,
    /// Independently collected account email.
    pub email: Option<String>,
    /// One-use email verification bound to the invitation.
    pub verification_id: Option<String>,
}

type Caller = (AccountId, InstallationId);
type Issued = (AccountId, InstallationId, String, Vec<String>);

fn invalid() -> Error {
    Error::invalid("invalid provider configuration or identity")
}

fn missing() -> Error {
    Error::new(ErrorCode::NotFound, "provider unavailable")
}

fn conflict(message: &str) -> Error {
    Error::new(ErrorCode::Conflict, message)
}

fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn validate(provider: &OidcProvider) -> Result<()> {
    let issuer = url::Url::parse(&provider.issuer).map_err(|_| invalid())?;
    let valid_issuer = issuer.scheme() == "https"
        && issuer.host_str().is_some()
        && issuer.username().is_empty()
        && issuer.password().is_none()
        && issuer.query().is_none()
        && issuer.fragment().is_none();
    let valid_preset = match provider.preset.as_str() {
        "apple" => provider.issuer == "https://appleid.apple.com",
        "google" => provider.issuer == "https://accounts.google.com",
        "generic" => !matches!(
            issuer.host_str(),
            Some("appleid.apple.com" | "accounts.google.com")
        ),
        _ => false,
    };
    let valid_scope = provider.scopes.len() <= 10
        && provider.scopes.iter().all(|scope| {
            !scope.is_empty()
                && scope.len() <= 100
                && scope
                    .bytes()
                    .all(|b| b.is_ascii_graphic() && !matches!(b, b'"' | b'\\'))
        });
    let valid_claim = provider.display_name_claim.as_deref().is_none_or(|claim| {
        matches!(
            claim,
            "name" | "given_name" | "preferred_username" | "nickname"
        )
    });
    if identifier(&provider.id)
        && valid_issuer
        && valid_preset
        && valid_scope
        && valid_claim
        && !provider.client_id.is_empty()
        && provider.client_id.len() <= 512
        && !provider.client_id.chars().any(char::is_control)
        && !provider.label.trim().is_empty()
        && provider.label.len() <= 64
        && !provider.label.chars().any(char::is_control)
    {
        Ok(())
    } else {
        Err(invalid())
    }
}

fn binding(provider: &OidcProvider) -> Result<Vec<u8>> {
    serde_json::to_vec(&(&provider.id, &provider.issuer, &provider.client_id))
        .map_err(Error::internal)
}

fn require_admin(tx: &Transaction<'_>, inner: &crate::Inner, caller: Caller) -> Result<()> {
    session::check_fresh(tx, &inner.config, caller.0, caller.1, false)?;
    let allowed: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users u JOIN role_capabilities c ON c.role=u.role
        WHERE u.id=?1 AND u.disabled_at IS NULL AND c.capability=?2)",
            params![
                caller.0.to_string(),
                calternal_types::Capability::ServerSettings.name()
            ],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if !allowed {
        return Err(Error::new(
            ErrorCode::Forbidden,
            "server settings capability required",
        ));
    }
    Ok(())
}

fn check_provider(c: &Connection, id: &str, revision: &str) -> Result<()> {
    let active: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM oidc_providers WHERE id=?1 AND revision=?2 AND enabled=1)",
            params![id, revision],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if !active {
        return Err(missing());
    }
    Ok(())
}

fn revoke_sessions(
    tx: &Transaction<'_>,
    id: &str,
    user: Option<AccountId>,
) -> Result<Vec<InstallationId>> {
    let mut stmt = tx.prepare_cached("UPDATE installations SET revoked_at=?3 WHERE revoked_at IS NULL AND id IN
        (SELECT installation_id FROM oidc_sessions WHERE provider_id=?1 AND (?2 IS NULL OR user_id=?2))
        RETURNING id").map_err(sql)?;
    stmt.query_map(params![id, user.map(|u| u.to_string()), now_ms()], |r| {
        r.get::<_, String>(0)
    })
    .map_err(sql)?
    .map(|id| id.map_err(sql)?.parse().map_err(Error::internal))
    .collect()
}

fn usable_alternative(c: &Connection, provider: &str, user: Option<AccountId>) -> Result<bool> {
    c.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM oidc_links l JOIN users u ON u.id=l.user_id
        WHERE l.provider_id=?1 AND (?2 IS NULL OR l.user_id=?2) AND u.disabled_at IS NULL
        AND NOT EXISTS(SELECT 1 FROM passkeys k WHERE k.user_id=l.user_id)
        AND NOT EXISTS(SELECT 1 FROM oidc_links alt JOIN oidc_providers p ON p.id=alt.provider_id
                       WHERE alt.user_id=l.user_id AND p.id<>l.provider_id AND p.enabled=1))",
        params![provider, user.map(|u| u.to_string())],
        |r| r.get(0),
    )
    .map_err(sql)
}

fn issue(
    tx: &Transaction<'_>,
    inner: &crate::Inner,
    user: AccountId,
    provider: &str,
    subject: &str,
    device: &str,
    asserted: bool,
) -> Result<(InstallationId, String)> {
    let ttl = inner.config.session_ttl.as_millis() as i64;
    let out = session::create(
        tx,
        user,
        session::InstallationKind::Browser,
        device,
        ttl,
        asserted,
    )?;
    tx.execute(
        "INSERT INTO oidc_sessions(installation_id,provider_id,subject,user_id) VALUES(?1,?2,?3,?4)",
        params![out.0.to_string(), provider, subject, user.to_string()],
    )
    .map_err(sql)?;
    Ok(out)
}

impl Auth {
    /// Lists enabled provider presentation settings or all settings for an administrator.
    /// No credential is opened for this response.
    pub async fn oidc_providers(&self, admin: Option<Caller>) -> Result<Vec<(OidcProvider, bool)>> {
        let inner = Arc::clone(&self.inner);
        self.inner.db.run(move |c| {
            let tx = c.transaction().map_err(sql)?;
            if let Some(caller) = admin { require_admin(&tx,&inner,caller)?; }
            let mut stmt = tx.prepare_cached("SELECT config,enabled FROM oidc_providers WHERE enabled=1 OR ?1 ORDER BY id").map_err(sql)?;
            stmt.query_map([admin.is_some()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,bool>(1)?))).map_err(sql)?
                .map(|row| {
                    let (config,enabled) = row.map_err(sql)?;
                    Ok((serde_json::from_str(&config).map_err(Error::internal)?,enabled))
                }).collect()
        }).await
    }

    /// Saves a provider after fresh server-admin proof. A new provider is disabled.
    /// The credential is sealed by the deployment secrets service in the same commit.
    pub async fn configure_oidc(
        &self,
        caller: Caller,
        provider: OidcProvider,
        secret: Option<SecretValue>,
    ) -> Result<()> {
        validate(&provider)?;
        let inner = Arc::clone(&self.inner);
        self.inner
            .db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sql)?;
                require_admin(&tx, &inner, caller)?;
                let existing = tx
                    .query_row(
                        "SELECT issuer,client_id,secret FROM oidc_providers WHERE id=?1",
                        [&provider.id],
                        |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, Vec<u8>>(2)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(sql)?;
                if let Some((issuer, client, _)) = &existing
                    && (issuer != &provider.issuer || client != &provider.client_id)
                {
                    return Err(conflict(
                        "remove the existing configuration before changing issuer or client ID",
                    ));
                }
                if existing.is_none() {
                    let (number, duplicate): (i64, bool) = tx
                        .query_row(
                            "SELECT count(*),COALESCE(max(issuer=?1),0) FROM oidc_providers",
                            [&provider.issuer],
                            |r| Ok((r.get(0)?, r.get(1)?)),
                        )
                        .map_err(sql)?;
                    if number >= 16 || duplicate {
                        return Err(conflict("provider limit or issuer already configured"));
                    }
                }
                let updating = existing.is_some();
                let ciphertext = match secret {
                    Some(secret) => inner
                        .invite_key
                        .oidc_cipher()
                        .seal(&binding(&provider)?, &secret)
                        .map_err(Error::internal)?,
                    None => existing.map(|(_, _, secret)| secret).ok_or_else(invalid)?,
                };
                let encoded = serde_json::to_string(&provider).map_err(Error::internal)?;
                let revision = uuid::Uuid::now_v7().to_string();
                // Plain UPDATE respects the recovery trigger's dirty-key deduplication.
                if updating {
                    tx.execute(
                        "UPDATE oidc_providers SET config=?2,secret=?3,revision=?4 WHERE id=?1",
                        params![provider.id, encoded, ciphertext, revision],
                    )
                    .map_err(sql)?;
                } else {
                    tx.execute(
                        "INSERT INTO oidc_providers(id,issuer,client_id,config,secret,revision,enabled) VALUES(?1,?2,?3,?4,?5,?6,0)",
                        params![
                            provider.id,
                            provider.issuer,
                            provider.client_id,
                            encoded,
                            ciphertext,
                            revision
                        ],
                    )
                    .map_err(sql)?;
                }
                account_recovery::audit(
                    &tx,
                    caller.0,
                    "oidc",
                    "provider_configured",
                    "server-admin",
                    now_ms(),
                )?;
                spaces::commit(tx)
            })
            .await
    }

    /// Enables, disables or removes a provider and revokes its affected sessions.
    /// Without verified operator recovery, refuse to strand a provider-only account.
    pub async fn set_oidc_provider_state(
        &self,
        caller: Caller,
        id: &str,
        action: &str,
        operator_recovery: bool,
    ) -> Result<()> {
        if !matches!(action, "enable" | "disable" | "remove") {
            return Err(invalid());
        }
        let inner = Arc::clone(&self.inner);
        let (id, action) = (id.to_owned(), action.to_owned());
        self.inner
            .db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sql)?;
                require_admin(&tx, &inner, caller)?;
                if action == "enable" {
                    let preset: String = tx
                        .query_row(
                            "SELECT json_extract(config, '$.preset') FROM oidc_providers WHERE id=?1",
                            [&id],
                            |row| row.get(0),
                        )
                        .optional()
                        .map_err(sql)?
                        .ok_or_else(missing)?;
                    if preset != "generic" {
                        return Err(conflict("this provider preset is pending verification"));
                    }
                }
                if action != "enable" && !operator_recovery && !usable_alternative(&tx, &id, None)?
                {
                    return Err(conflict(
                        "some accounts need another sign-in method or operator recovery",
                    ));
                }
                let installations = if action == "enable" {
                    Vec::new()
                } else {
                    revoke_sessions(&tx, &id, None)?
                };
                let changed = if action == "remove" {
                    tx.execute("DELETE FROM oidc_providers WHERE id=?1", [&id])
                } else {
                    tx.execute(
                        "UPDATE oidc_providers SET enabled=?2,revision=?3 WHERE id=?1",
                        params![id, action == "enable", uuid::Uuid::now_v7().to_string()],
                    )
                }
                .map_err(sql)?;
                if changed != 1 {
                    return Err(missing());
                }
                account_recovery::audit(&tx, caller.0, "oidc", &action, "server-admin", now_ms())?;
                spaces::commit_with(tx, || inner.sessions.forget_installations(installations))
            })
            .await
    }

    /// Returns enabled provider metadata and the generation bound to new flows.
    /// Reads one protected row without opening the client secret.
    pub async fn oidc_provider(&self, id: &str) -> Result<(OidcProvider, String)> {
        let id = id.to_owned();
        self.inner
            .db
            .run(move |c| {
                let (config, revision) = c
                    .query_row(
                        "SELECT config,revision FROM oidc_providers WHERE id=?1 AND enabled=1",
                        [id],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                    )
                    .optional()
                    .map_err(sql)?
                    .ok_or_else(missing)?;
                Ok((
                    serde_json::from_str(&config).map_err(Error::internal)?,
                    revision,
                ))
            })
            .await
    }

    /// Opens a provider credential for host-only exchange after configuration recheck.
    /// Return it only to the endpoint authorized by the trusted issuer.
    pub async fn oidc_credential(
        &self,
        provider: &OidcProvider,
        revision: &str,
    ) -> Result<SecretValue> {
        let id = provider.id.clone();
        let revision = revision.to_owned();
        let binding = binding(provider)?;
        let inner = Arc::clone(&self.inner);
        self.inner.db.run(move |c| {
            let ciphertext:Vec<u8> = c.query_row("SELECT secret FROM oidc_providers WHERE id=?1 AND revision=?2 AND enabled=1",
                params![id,revision],|r|r.get(0)).optional().map_err(sql)?.ok_or_else(missing)?;
            inner.invite_key.oidc_cipher().open(&binding,&ciphertext).map_err(Error::internal)
        }).await
    }

    /// Reads the provider identity associated with this live session, if any.
    /// Passkey sessions have no provider row.
    pub async fn oidc_session(&self, caller: Caller) -> Result<Option<OidcSession>> {
        self.inner
            .db
            .run(move |c| {
                c.query_row(
                    "SELECT p.provider_id,p.subject FROM oidc_sessions p JOIN installations i
                ON i.id=p.installation_id WHERE i.id=?1 AND i.user_id=?2 AND i.revoked_at IS NULL",
                    params![caller.1.to_string(), caller.0.to_string()],
                    |r| {
                        Ok(OidcSession {
                            provider: r.get(0)?,
                            subject: r.get(1)?,
                        })
                    },
                )
                .optional()
                .map_err(sql)
            })
            .await
    }

    /// Supplies enabled or disabled provider links for an authenticated account.
    pub async fn oidc_links(&self, account: AccountId) -> Result<Vec<String>> {
        self.inner
            .db
            .run(move |c| {
                let mut stmt = c
                    .prepare_cached(
                        "SELECT provider_id FROM oidc_links WHERE user_id=?1 ORDER BY provider_id",
                    )
                    .map_err(sql)?;
                stmt.query_map([account.to_string()], |r| r.get(0))
                    .map_err(sql)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(sql)
            })
            .await
    }

    /// Completes sign-in or explicit Linking from a host-validated OIDC subject.
    /// An unknown subject requires a valid invitation, consumed with account creation.
    /// The caller has already validated issuer, audience, nonce, code and signature.
    #[allow(clippy::too_many_arguments)]
    pub async fn finish_oidc(
        &self,
        provider: String,
        revision: String,
        subject: String,
        display_name: Option<String>,
        signup: Option<OidcSignup>,
        link: Option<Caller>,
        reverify: Option<Caller>,
        device: String,
    ) -> Result<Option<Issued>> {
        if subject.is_empty() || subject.len() > 255 || subject.chars().any(char::is_control) {
            return Err(invalid());
        }
        let signup = if let Some(s) = signup {
            let name = s.name.or(display_name).ok_or_else(invalid)?;
            let mut invite = crate::setup::check(&self.inner, &s.token, &name).await?;
            if !invite.invite {
                return Err(crate::invites::invalid());
            }
            let username = account_details::username(&s.username)?;
            let email = s
                .email
                .as_deref()
                .map(crate::account_email::email)
                .transpose()?;
            if self.inner.config.email_required && (email.is_none() || s.verification_id.is_none())
                || s.verification_id.is_some() && email.is_none()
            {
                return Err(invalid());
            }
            invite.signup = Some(crate::setup::Signup {
                username,
                email,
                verification: s.verification_id,
            });
            Some(invite)
        } else {
            None
        };
        let inner = Arc::clone(&self.inner);
        self.inner.db.run(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql)?;
            check_provider(&tx,&provider,&revision)?;
            let selected:Option<String> = tx.query_row("SELECT l.user_id FROM oidc_links l JOIN users u ON u.id=l.user_id
                WHERE l.provider_id=?1 AND l.subject=?2 AND u.disabled_at IS NULL",params![provider,subject],|r|r.get(0))
                .optional().map_err(sql)?;
            if let Some((user,installation)) = link {
                session::check_fresh(&tx,&inner.config,user,installation,false)?;
                if selected.as_deref().is_some_and(|id|id!=user.to_string()) || signup.is_some() {
                    return Err(conflict("this identity is already linked"));
                }
                let changed = tx.execute("INSERT INTO oidc_links(provider_id,subject,user_id) VALUES(?1,?2,?3) ON CONFLICT DO NOTHING",
                    params![provider,subject,user.to_string()]).map_err(sql)?;
                if changed==0 && selected.is_none() { return Err(conflict("another identity is already linked for this provider")); }
                account_recovery::audit(&tx,user,"oidc","linked","account",now_ms())?;
                spaces::commit(tx)?;
                return Ok(None);
            }
            let now = now_ms();
            let (user,codes) = if let Some(id)=selected {
                if signup.is_some() { return Err(conflict("this identity already has an account")); }
                (AccountId::parse(&id).map_err(Error::internal)?,Vec::new())
            } else if let Some(signup)=signup.filter(|_| reverify.is_none()) {
                let hash = signup.token_hash.as_slice().try_into().map_err(Error::internal)?;
                let grant = crate::invites::find(&tx,hash,now)?.ok_or_else(crate::invites::invalid)?;
                crate::setup::ensure_person_name_available(&tx,&signup.name,None,false)?;
                crate::setup::insert_signup(&tx,&signup,"",now)?;
                let user=signup.user;
                crate::invites::consume(&tx,grant,user,now)?;
                tx.execute("INSERT INTO oidc_links(provider_id,subject,user_id) VALUES(?1,?2,?3)",params![provider,subject,user.to_string()]).map_err(sql)?;
                let codes=account_details::rotate_codes(&tx,&inner.invite_key,user)?;
                account_recovery::audit(&tx,user,"oidc","enrolled","account",now)?;
                (user,codes)
            } else {
                return Err(Error::new(ErrorCode::SetupTokenInvalid,"an invitation is required to create an account"));
            };
            if let Some((expected,installation))=reverify {
                let matching:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM oidc_sessions o JOIN installations i ON i.id=o.installation_id
                    WHERE o.installation_id=?1 AND o.user_id=?2 AND o.provider_id=?3 AND o.subject=?4 AND i.revoked_at IS NULL
                    AND (i.expires_at IS NULL OR i.expires_at>?5))",
                    params![installation.to_string(),expected.to_string(),provider,subject,now],|r|r.get(0)).map_err(sql)?;
                if user!=expected || !matching { return Err(invalid()); }
            }
            let (installation,token)=issue(&tx,&inner,user,&provider,&subject,&device,reverify.is_some())?;
            spaces::commit(tx)?;
            Ok(Some((user,installation,token,codes)))
        }).await
    }

    /// Unlinks a provider explicitly after fresh proof and revokes its sessions.
    /// Refuses removing the last usable authenticator.
    pub async fn unlink_oidc(&self, caller: Caller, provider: &str) -> Result<()> {
        let inner = Arc::clone(&self.inner);
        let provider = provider.to_owned();
        self.inner
            .db
            .run(move |c| {
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sql)?;
                session::check_fresh(&tx, &inner.config, caller.0, caller.1, false)?;
                if !usable_alternative(&tx, &provider, Some(caller.0))? {
                    return Err(conflict("add another sign-in method first"));
                }
                let installs = revoke_sessions(&tx, &provider, Some(caller.0))?;
                let changed = tx
                    .execute(
                        "DELETE FROM oidc_links WHERE provider_id=?1 AND user_id=?2",
                        params![provider, caller.0.to_string()],
                    )
                    .map_err(sql)?;
                if changed != 1 {
                    return Err(missing());
                }
                account_recovery::audit(&tx, caller.0, "oidc", "unlinked", "account", now_ms())?;
                spaces::commit_with(tx, || inner.sessions.forget_installations(installs))
            })
            .await
    }

    /// Whether the authenticated account still has no enrolled passkey.
    pub async fn offer_first_passkey(&self, account: AccountId) -> Result<bool> {
        self.inner
            .db
            .run(move |c| {
                c.query_row(
                    "SELECT NOT EXISTS(SELECT 1 FROM passkeys WHERE user_id=?1)",
                    [account.to_string()],
                    |r| r.get(0),
                )
                .map_err(sql)
            })
            .await
    }
}
