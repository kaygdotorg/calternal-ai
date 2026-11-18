# Account onboarding and recovery transport

The host serves these routes under `/api/v1/auth`. Mutations use JSON POST
requests with the existing origin and session checks. Challenges and grants
are opaque and temporary. Responses containing secrets use `Cache-Control:
no-store`. Never put those values into a URL, telemetry, or the operation
receipt pipeline. Times are Unix milliseconds.

| Method and route | Request | Success |
| --- | --- | --- |
| GET /status | | Existing setup and sign-in flags, plus `email_required` and `mail_available` |
| POST /email/start | `{email, token?}` | `{challenge_id, expires_at}` |
| POST /email/confirm | `{challenge_id, code, token?}` | `{verification_id?, email_status}` |
| POST /setup/start | `{token, name, username, email?, verification_id?}` | `{ceremony, options}` |
| POST /setup/finish | `{ceremony, credential, device?}` | Session cookie and `{user, recovery_codes, second_passkey_prompt}` |
| GET /me | Session cookie | Existing `{user, installation}` with username on user, plus `{email, email_status, mail_available, second_passkey_prompt, recovery_codes_configured}` |
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

Recovery uses `code` or `email` as the method. On the email path the first
proof starts the hold. Status reports `proof`, `waiting` or `claim`. The first
status request after the wait can send the claim code. Veto uses fresh passkey
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
