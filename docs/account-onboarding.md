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

OIDC is off by default. To make the host routes available, set
`CALTERNAL_ACCOUNT_OIDC_ENABLED=1` for the server and restart it. The
existing auth schema already includes provider tables. A new provider is
disabled until a server admin explicitly enables it. Apple and Google may
be configured for future testing and cannot be enabled in this release.

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
display_name_claim?}` with a fresh sign-in check. Supply the client secret
through the private admin secret input over HTTPS. The host seals it using
the deployment secrets service and omits it from reads. Enable the saved
provider by sending `{provider_id, action:"enable"}` to `/oidc/admin/state`.
The public provider list contains enabled definitions only.

For Authentik or another generic provider, create a confidential web
client with authorization code flow, S256 PKCE and the exact callback
`https://<calternal-host>/api/v1/auth/oidc/callback`. Use that provider's
exact HTTPS issuer, including any path and trailing slash. Discovery,
authorization, token and JWKS endpoints stay on the issuer's origin.
The host always requests `openid`. Add `profile` if you want to suggest a
display name. The scope and display-name mapping grant no local authority.
For a private CA, make its PEM bundle available to the server and set
`CALTERNAL_OIDC_CA_FILE` to its path. TLS still verifies the hostname.

A provider session can read the account immediately. For sensitive
actions the generic flow requests `prompt=login` and `max_age=0`. The
signed ID token must include `auth_time` within the preceding two minutes
with at most 30 seconds of future clock skew. The same issuer and subject
must match the original session. A missing or stale claim does not renew
the proof.

Provider changes expire in-flight flows. Disablement revokes sessions
minted through the provider. Removal also deletes identity links and the
sealed credential. A provider-only account needs another sign-in method
or an explicit operator recovery path before disabling or removing its
provider. No plugin manifest or worker participates in this flow.

### Owner setup for Apple and Google testing

Use a disposable Calternal HTTPS origin for real-provider testing. For the
production origin, the exact registered redirect is
`https://calternal.cloud/api/v1/auth/oidc/callback`. Each provider needs
the exact callback for whichever origin runs the test.

- **Apple** — In Apple Developer, enable Sign in with Apple on a primary
  App ID. Create a **Services ID** for the web client, select that primary
  App ID, and register the app domain and the exact return URL. The
  Services ID is the client ID. Create a Sign in with Apple key for the
  same primary App ID. Keep the downloaded key private and use it to sign
  an ES256 JWT. Set `kid` in the header to the key ID, `iss` to the
  Team ID, `sub` to the Services ID, `aud` to `https://appleid.apple.com`,
  and `iat` and `exp` to Unix seconds with a lifetime under six months.
  Supply this JWT through the private admin secret input. Leave the
  preset's additional scopes empty. Calternal asks for its recovery
  email separately. Apple web reverification still needs a real proof of
  recent authentication. [Web configuration](https://developer.apple.com/help/account/capabilities/configure-sign-in-with-apple-for-the-web/)
  · [Client authentication setup](https://developer.apple.com/documentation/signinwithapple/configuring-your-environment-for-sign-in-with-apple)
- **Google** — In Google Auth Platform, select a project, configure consent
  and use a test user if the application is in Testing. Create an OAuth
  client of type **Web application**. Register the exact callback under
  **Authorized redirect URIs**. Request `openid profile` and supply the
  client ID and secret through the admin inputs. A sign-in smoke test can
  use Testing. To receive the signed `auth_time` claim, publish and verify
  the app, then enable **Settings → Advanced Settings → Session age
  claims**. The real-provider check requests the claim and must show that
  Google returns a current value before the preset can be enabled.
  [Google OpenID Connect setup](https://developers.google.com/identity/openid-connect/openid-connect)
  · [Session age claim setup](https://developers.google.com/identity/siwg/security-bundle)
