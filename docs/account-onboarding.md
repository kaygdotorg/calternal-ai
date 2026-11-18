# Account onboarding and recovery transport

The host serves these routes under `/api/v1/auth`. Mutations use JSON POST
requests with the existing origin and session checks. Challenges and grants
are opaque and temporary. Responses containing secrets use `Cache-Control:
no-store`. Never put those values into a URL, telemetry, or the operation
receipt pipeline. Times are Unix milliseconds.

| Method and route | Request | Success |
| --- | --- | --- |
| GET /status | | Existing setup and sign-in flags, plus `email_required` and `mail_available` |
| POST /email/start | `{email, token?}` | `{challenge_id, expires_at}`, or `{email_status: unverified}` for a no-mail address edit |
| POST /email/confirm | `{challenge_id, code, token?}` | `{verification_id?, email_status}` |
| POST /setup/start | `{token, name, username, email?, verification_id?}` | `{ceremony, options}` |
| POST /setup/finish | `{ceremony, credential, device?}` | Session cookie and `{user, recovery_codes, second_passkey_prompt}` |
| GET /me | Session cookie | Existing `{user, installation}` with username on user, plus `{email, email_status, mail_available, second_passkey_prompt, recovery_codes_configured, email_recovery_pending, email_setup_required}` |
| POST /username/change | `{username}` | `{username}` |
| POST /recovery/codes/regenerate | `{}` | `{recovery_codes}` |
| POST /passkeys/prompt/dismiss | `{}` | `{second_passkey_prompt: false}` |
| POST /recovery/start | `{method, email?}` | `{flow_id}` |
| POST /recovery/prove | `{flow_id, code}` | `{hold_until}` or `{enrollment_grant}` |
| POST /recovery/status | `{flow_id}` | `{stage, hold_until?}` |
| POST /recovery/veto | `{}` | `{cancelled}` |
| POST /recovery/enroll/start | `{enrollment_grant}` | `{ceremony, options}` |
| POST /recovery/enroll/finish | `{ceremony, credential, device?}` | `{sign_in_required: true}` |
| GET /recovery/history | Session cookie | `{events: [{method, action, at, actor}]}` |

Email status is `missing`, `unverified` or `verified`. Signup supplies the
setup or invite token on both email calls. Signed-in address change omits it
and requires recent passkey proof. An email installation can be configured to
require verification at signup. On a server without SMTP, email is optional and
the account can save an unverified address during setup or address change.
A previously verified address is kept until mail can verify its replacement.

After enabling the email policy, existing accounts can still sign in. The
passkey `/login/finish` response and `/me` include `email_setup_required`
when policy requires verification and the account has no verified address.
Use it to offer the email step after accepting the session. With no mail
configured, `mail_available: false` lets Account show “Email cannot be
verified because this server has no mail configured.” The policy switch
changes configuration only.


Recovery uses `code` or `email` as the method. On the email path the first
proof starts the hold. Status reports `proof`, `waiting` or `claim`. The first
status request after the wait can send the claim code. Veto uses fresh sign-in
proof and cancels the caller's pending email recovery. Local operator resets
hand out an enrollment grant directly. Replacement enrollment returns the
user to passkey sign-in. The first successful assertion after replacement
issues a fresh code set once. The existing passkey and installation APIs
continue to list and manage registered credentials and signed-in devices.

## UI validation

- Usernames are 3–32 ASCII letters, digits or single interior hyphens. The
  server trims surrounding whitespace and lowercases letters. Availability
  is case-insensitive. UUID-shaped names and `admin`, `administrator`,
  `root`, `owner`, `system`, `support` and `calternal` are reserved.
- Email verification codes are eight decimal digits, including leading
  zeroes. Each code lasts 10 minutes and permits five incorrect tries.
  Hosted onboarding requires a verified email. If sending fails, offer
  resend and a different email address. Tell the person that verification
  is required to continue. Self-hosted onboarding can omit the address.
- Show ten single-use recovery codes once. Each is 32 hexadecimal
  characters (128 bits). Treat them as text so leading zeroes are preserved.
  Offer to save the displayed set. Regenerating invalidates the previous set.

## Operator configuration

Set `CALTERNAL_ACCOUNT_EMAIL_REQUIRED=true` on hosted installations. An
optional self-hosted mail relay uses `CALTERNAL_SMTP_HOST`,
`CALTERNAL_SMTP_FROM`, `CALTERNAL_SMTP_USERNAME`, and the runtime-injected
`CALTERNAL_SMTP_PASSWORD`. Set `CALTERNAL_SMTP_TLS=starttls` for mandatory
STARTTLS. The default is implicit TLS. An optional `CALTERNAL_SMTP_PORT`
overrides the corresponding submission port. Leave SMTP host unset for
no-mail mode. Inject the password directly at runtime and keep it out of
deployment files, command arguments and logs.

A local operator verifies the owner's identity through a trusted outside
channel and runs `calternal account-recover UUID --reason lost_passkeys
--identity-verified --appdata PATH` with the pinned installation ID. The
command needs a private terminal and the running server's Unix user. The
grant expires in 15 minutes and only authorizes replacement-passkey
enrollment. Give the grant to the verified owner over the trusted channel.

## Core OpenID Connect

The fresh baseline includes the provider tables. Its server keeps the OIDC
transport disabled. The following contract is for the provider rollout.

These routes share the protected auth prefix and use the same private
response and JSON mutation rules as passkey sign-in.

| Method and route | Request | Success |
| --- | --- | --- |
| GET /oidc/providers | | `{providers: [{id, label, icon}]}` |
| POST /oidc/start | `{provider_id, purpose, signup?}` | `{flow_id, redirect_url}` |
| GET or POST /oidc/callback | Provider authorization response | Redirect to `/?account-oidc=complete` |
| POST /oidc/finish | `{flow_id, device?}` | Session cookie and `{user, offer_passkey, recovery_codes}`, or `{linked: true}` |
| POST /oidc/unlink | `{provider_id}` | `{unlinked: true}` |
| GET /oidc/admin/providers | Fresh server admin proof | Provider configuration without secrets, enabled state and presets |
| POST /oidc/admin/providers | `{provider, client_secret?}` | `{saved: true}` |
| POST /oidc/admin/state | `{provider_id, action, operator_recovery?}` | `{saved: true}` |

Use `sign_in`, `link` or `reverify` as the purpose. Save the flow ID in
tab-local state before navigating to the provider, then submit it to
`/oidc/finish` after the callback. An optional signup draft has
`{token, name?, username, email?, verification_id?}`. Hosted support
requires a verified address independent of provider claims. An unknown
identity without an invite returns `invite_required`. Linked accounts use
the issuer and subject sent by the provider. The host never matches email
or username. Settings links require a live session and fresh proof.
`GET /me` reports `sign_in_provider` (null for passkey), and
`provider_links`. When a sensitive action requests reverification, use
that same method. A provider-first success can offer the first passkey and
the person can skip it.

An admin configures `{id, preset, label, issuer, client_id, scopes,
display_name_claim?}`. The Apple and Google presets supply their issuers.
The generic preset supports Authentik and other HTTPS issuers with
same-origin authorization, token and JWKS endpoints and advertised S256
PKCE. Apple uses its documented confidential-client code flow with state,
browser binding and a signed nonce. Supply the current Apple client-secret
JWT through the admin secret field. The host seals client credentials,
owns the token exchange and emits only identity and the host session.
The scope and display-name mapping never grant local permissions. [Apple
discovery metadata](https://appleid.apple.com/.well-known/openid-configuration)

Provider changes expire in-flight flows. Disablement revokes sessions
minted through the provider. Removal also deletes identity links and the
sealed credential. A provider-only account needs another sign-in method
or an explicit operator recovery path before disabling or removing its
provider. No plugin manifest or worker participates in this flow.
