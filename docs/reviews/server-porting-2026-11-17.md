# Trusted HTTP/account and address port

Source: `/home/kayg/Developer/calternal`, commit
`ea57b84b9c9e40e8b67b7d1e1f1805915123b040` (`origin/dev` at assignment).
All source reads used git objects only. No source checkout, modification or build.

| Source path:lines | Port and intentional differences |
|---|---|
| `crates/auth/src/http.rs:1–277,390–425,486–665` | `src/account_http.rs` and merged server guard: JSON/64 KiB, exact configured origin, discoverable setup/login, reverification, no-store, Secure/HttpOnly/SameSite=Lax host cookie, bearer selection, duplicate-cookie rejection, logout without expiring newer cookies. Uses this workspace's Auth/error adapter; returns source catalogue messages and retryability without private diagnostics. |
| `crates/server/src/error.rs:1–69`; `crates/core/src/error.rs:106–223` | Preserve account HTTP `{error:{code,message,retryable,request_id}}` and safe catalogue text independently of this preview's existing operation envelope. Unknown setup candidates still use generic `invite_invalid` regardless of token syntax; enrollment redemption failures use `setup_token_invalid`. No invite implementation is introduced. |
| `crates/auth/src/http.rs:595–648` | On the user's later 2026-11-17 no-compatibility instruction, omit obsolete `calternal_session` recognition/clearing. Only `__Host-calternal_session` participates. Duplicate current-cookie and delayed logout/refusal protections remain. Two source compatibility tests are retained but explicitly ignored as superseded requirements. |
| `crates/auth/src/http.rs:540–590` | Transport `ConnectInfo` source, ignored XFF unless the socket peer is trusted, right-to-left chain walk stopping at first untrusted hop, malformed-chain rejection, mapped IPv4 and IPv6 /64 buckets. Real listener now supplies ConnectInfo. Proxy CIDRs are explicit CLI configuration, empty by default. |
| `crates/auth/src/passkey.rs:107–212`; `http.rs:148–258` | Missing library adapters in `auth/src/http_facing.rs`, internal `ceremony::setup_account`, and `passkey::finish_login_inner`. Setup finishes derive the account from protected prompt state. Anonymous login binds to an internal, never-persisted nil account sentinel until the signed answer identifies its account; existing library callers retain strict account binding. Reverify has a separate start route binding both current account and purpose, per the port-map contract. Wrong purpose cannot consume a rightful prompt. Login admission occurs before body parsing, as in the source. |
| `crates/auth/src/account.rs:92–130`; `passkeys.rs:260–345` | Trusted HTTP list/revoke installations and list/add/rename/remove passkeys, using this library's existing account-bound primitives. Preserve `current`, passkey labels, recent-proof checks, and transactional last-key guard. Old Rust Plugin/engine ActionSpec wrappers become explicit trusted routes, not tenant plugins. Successful payloads preserve the source action result fields without its engine envelope. |
| `crates/auth/src/http.rs:280–388`; `account.rs:123–128`; `passkeys.rs:306–342` | Do not import old engine journal/live-feed dependencies. Account changes never publish into a space journal. Protected account event publication and durable mutation receipt replay remain unfinished; the complete adapted source revoke/retry test stays ignored with its assertions intact. No idempotency promise is made for these new account POSTs. |
| `crates/auth/src/http.rs:441–483,667–787` | DAV/app-password minting, route-specific scopes, websocket revalidation and old server Authenticator trait are not in this lane. Only existing non-DAV session identify is accepted. API calls reidentify sessions and requery Directory authority. There is no websocket/journal endpoint to port live tests onto. |
| `crates/auth/tests/host_cookies.rs:8–173` | All 3 cases brought across. Duplicate host headers/stale requests runs. Obsolete-name cleanup and schema-six upgrade are ignored under the later no-compatibility decision. The old root API in the retained fixture becomes the current single state-root Auth API; v16 fixture tables are removed before its downgrade. No runtime schema compatibility adapter was added. |
| `crates/auth/tests/login.rs:11–79,146–229,296–316` | 8 applicable cases brought across. 7 run; revoke/retry remains ignored because protected account receipt replay is missing. Its full original retry assertions remain. |
| `crates/auth/tests/login.rs:86–142,233–292` | 3 live journal/websocket cases not ported: the private input SSE feed does not publish those auth events and there is no websocket. They are not counted as executable or passing ports. |
| `crates/auth/tests/isolation.rs:1–218` | All 3 cases brought across. Source account actions map to trusted routes in the fixture; journal/live/hello authentication probes use this server's authenticated describe route. Actual websocket delivery isolation is not claimed. Foreign/made-up IDs still produce equal outcomes and leave the rightful owner's credentials and prompts intact. |
| `crates/auth/tests/abuse.rs:1–455` | All 12 cases brought across. The source invite-creation engine fixture is replaced by privileged Auth::create for the same independent Setup/Add/Login pool checks. Authentication probes target describe. Source rate-limit/XFF/forged-session/garbage credential/unknown-field assertions remain. |
| `crates/auth/src/http.rs:795–804,937–978` | Host-only secure cookie and trusted-proxy tests adapted into `tests/account_transport.rs`; add IPv6/mapped-v4 checks. |
| `crates/auth/tests/common/mod.rs:126–338` | Software passkey and request helpers retained, with current AppState/Directory/Store construction, temp directories, and required Host headers. No engine, old server/core/plugin, testkit or legacy data-root dependency. |

**Source test accounting:** 28 executable cases ported (26 from the four named
suites plus 2 inline transport cases), **25 runnable, 3 ignored**. Two ignores are
superseded legacy compatibility requirements; one is unfinished account receipt
replay. Three source live-feed cases are explicitly unported, not passing tests.
New transport/resolver/startup tests are additional coverage, not source ports.

## Address adapter and development mode

`docs/addresses.md` is the authority, including the later `/collections/<id>` and
`/files/<id>` decision. `ResolveInput.kind` is an open string, not an enum. Unknown
well-formed kinds, including files/collections pending their providers, return
not-found. Only the existing notes provider is implemented. No collection storage,
space/kind collection route, old address route, readable suffix or URL alias was
added. Current `personal`/`shared` aliases belong to this preview's existing dev
fixtures, not the old app; they remain per the explicit lane instruction.

The store's fields/API are unowned. The resolver therefore probes each current
membership's root with `Store::get`, without a content listing or an inaccessible
root probe. Replacing it with a global-ID storage index remains a performance
integration task, without a wire change. It returns current capabilities and
independent owning-space UUID, checking grants before and after the lookup.
Updates also recognize explicit content.write-only capability sets, preserving
the existing independent-capability policy; public resolve requires space.read.
Policy checks at commit/replay/delivery hide lost item access as not-found.

Notes reads/history/updates and summarize derive the owning space from item ID;
a supplied nonempty compatibility space hint must match. Creation/collection
listing still requires a target space. Core schema and TypeScript exports mark
item hints optional; explicit empty/null hints are rejected. CLI get/update/history
omit the hint by default. UUID account and space types remain distinct.

The server's binary defaults to no development tokens and no seeded accounts.
`--development` is required to create/accept fixture bearer tokens, including on
restart with existing token files. Normal startup uses its configured passkey
origin for base_url; the default is localhost because WebAuthn rejects an IP RP.
Development bearer startup retains its existing numeric base_url and separately
reports passkey_origin. A local 0600 setup-token file supplies first enrollment
without exposing the secret in logs. TLS/public origin/proxy configuration is
explicit; neither Host nor XFF silently selects a public passkey origin.

No recovery coordinator, protected account journal, production root validation,
old-data importer or compatibility schema was implemented here. The concurrent
recovery lane's changes are not this lane's implementation or qualification.
