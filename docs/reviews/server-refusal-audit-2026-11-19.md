# Server refusal audit

Reviewed server uses of `Error::unavailable`, explicit HTTP 5xx responses,
and the common error-envelope status mapping on the web-push follow-up.
The locations below are relative to `crates/server/src/`.

## Corrected responses

| Location | HTTP status | Condition |
| --- | --- | --- |
| [account_input/db.rs:340](../../crates/server/src/account_input/db.rs#L340) | 429 | account live subscriber cap reached. Close a connection or raise CALTERNAL_LIVE_LISTENERS |
| [account_input/plugin_settings.rs:135](../../crates/server/src/account_input/plugin_settings.rs#L135) | 429 | personal setting receipt capacity reached |
| [account_input/plugin_settings.rs:151](../../crates/server/src/account_input/plugin_settings.rs#L151) | 409 | personal setting capacity reached |
| [api/appearance/db.rs:353](../../crates/server/src/api/appearance/db.rs#L353) | 409 | selected families leave no room in the shared font cache |
| [api/appearance/http.rs:78](../../crates/server/src/api/appearance/http.rs#L78) | 429 | font upload service is busy |
| [api/appearance/mod.rs:144](../../crates/server/src/api/appearance/mod.rs#L144) | 403 | outside font downloads are disabled |
| [api/appearance/mod.rs:149](../../crates/server/src/api/appearance/mod.rs#L149) | 429 | font installer is busy |
| [api/appearance/mod.rs:188](../../crates/server/src/api/appearance/mod.rs#L188) | 429 | font install rate limit reached |
| [api/dispatch.rs:565](../../crates/server/src/api/dispatch.rs#L565) | 404 | operation has no host binding in this preview |
| [api/plugin_slots/live.rs:129](../../crates/server/src/api/plugin_slots/live.rs#L129) | 429 | space live subscriber limit reached |
| [api/plugins.rs:48](../../crates/server/src/api/plugins.rs#L48) | 403 | extension request was refused |
| [api/search.rs:169](../../crates/server/src/api/search.rs#L169) | 409 | space access changed. Retry the search |
| [api/wallpapers/mod.rs:77](../../crates/server/src/api/wallpapers/mod.rs#L77) | 429 | wallpaper service is busy |
| [api/wallpapers/upstream.rs:121](../../crates/server/src/api/wallpapers/upstream.rs#L121) | 429 | wallpaper fetch limit reached |
| [live.rs:144](../../crates/server/src/live.rs#L144) | 429 | space live subscriber limit reached |
| [live_rooms.rs:95](../../crates/server/src/live_rooms.rs#L95) | 429 | live socket capacity is busy |
| [live_rooms.rs:282](../../crates/server/src/live_rooms.rs#L282) | 429 | socket room limit reached |
| [live_rooms/hub.rs:268](../../crates/server/src/live_rooms/hub.rs#L268) | 429 | live room capacity is busy. Retry shortly |
| [live_rooms/hub.rs:381](../../crates/server/src/live_rooms/hub.rs#L381) | 429 | live room initial state capacity is busy |
| [live_rooms/hub.rs:522](../../crates/server/src/live_rooms/hub.rs#L522) | 429 | live room output capacity is busy |
| [live_rooms/hub.rs:556](../../crates/server/src/live_rooms/hub.rs#L556) | 429 | live room state capacity is busy |
| [machines/readiness.rs:355](../../crates/server/src/machines/readiness.rs#L355) | 409 | the selected machine does not support the candidate backend |
| [machines/sessions.rs:108](../../crates/server/src/machines/sessions.rs#L108) | 429 | too many pending sessions |
| [machines/workers.rs:178](../../crates/server/src/machines/workers.rs#L178) | 429 | worker input queue is full |
| [machines/workers.rs:240](../../crates/server/src/machines/workers.rs#L240) | 429 | Plugin workers are busy. Retry shortly |
| [plugins/agent_access.rs:422](../../crates/server/src/plugins/agent_access.rs#L422) | 409 | this account has 1024 retained turn authority records |
| [plugins/job_streams.rs:81](../../crates/server/src/plugins/job_streams.rs#L81) | 429 | too many job subscribers. Retry shortly |
| [plugins/jobs.rs:366](../../crates/server/src/plugins/jobs.rs#L366) | 400 | jobs need a space UUID |
| [plugins/jobs.rs:516](../../crates/server/src/plugins/jobs.rs#L516) | 429 | too many active jobs in this space |
| [plugins/jobs/waits.rs:120](../../crates/server/src/plugins/jobs/waits.rs#L120) | 409 | job run limit reached |
| [plugins/jobs/waits.rs:143](../../crates/server/src/plugins/jobs/waits.rs#L143) | 429 | durable wait capacity reached |
| [plugins/jobs/waits.rs:471](../../crates/server/src/plugins/jobs/waits.rs#L471) | 429 | webhook verification capacity reached |
| [plugins/network/admission.rs:100](../../crates/server/src/plugins/network/admission.rs#L100) | 429 | Space network budget is exhausted |
| [plugins/network/db.rs:140](../../crates/server/src/plugins/network/db.rs#L140) | 409 | Space network preference limit reached |
| [plugins/requests.rs:98](../../crates/server/src/plugins/requests.rs#L98) | 429 | request limit reached for this account or thread |
| [plugins/worker.rs:181](../../crates/server/src/plugins/worker.rs#L181) | 429 | The plugin worker is handling its maximum concurrent invocations. Wait for one to finish before retrying |
| [plugins/worker/admission.rs:100](../../crates/server/src/plugins/worker/admission.rs#L100) | 429 | worker admission capacity exhausted |
| [plugins/worker/admission.rs:119](../../crates/server/src/plugins/worker/admission.rs#L119) | 429 | worker admission timed out. Retry shortly |
| [plugins/worker/admission.rs:155](../../crates/server/src/plugins/worker/admission.rs#L155) | 429 | the previous worker is still draining. Retry shortly |
| [plugins/worker/admission.rs:173](../../crates/server/src/plugins/worker/admission.rs#L173) | 429 | worker capacity is still draining. Retry shortly |
| [plugins/worker/admission.rs:293](../../crates/server/src/plugins/worker/admission.rs#L293) | 429 | extension worker input queue is full |
| [plugins/worker/admission.rs:352](../../crates/server/src/plugins/worker/admission.rs#L352) | 429 | extension workers are busy. Retry shortly |
| [provider_sign_in/inbox.rs:100](../../crates/server/src/provider_sign_in/inbox.rs#L100) | 429 | provider sign-in inbox is busy |
| [provider_sign_in/inbox.rs:133](../../crates/server/src/provider_sign_in/inbox.rs#L133) | 429 | Too many provider sign-ins for this account |
| [provider_sign_in/mod.rs:112](../../crates/server/src/provider_sign_in/mod.rs#L112) | 409 | configure a shared space machine first |
| [secrets.rs:165](../../crates/server/src/secrets.rs#L165) | 404 | operation has no host binding in this preview |
| [turns/space_execution.rs:94](../../crates/server/src/turns/space_execution.rs#L94) | 409 | the space Owner must configure a shared machine |
| [web_push/db.rs:206](../../crates/server/src/web_push/db.rs#L206) | 404 | Register this browser before sending a test |
| [web_push/db.rs:235](../../crates/server/src/web_push/db.rs#L235) | 429 | Register this browser or wait a minute before testing again |
| [web_push/mod.rs:175](../../crates/server/src/web_push/mod.rs#L175) | 409 | Web push requires an HTTPS server |
| [web_push/mod.rs:217](../../crates/server/src/web_push/mod.rs#L217) | 429 | Web push queue is busy |

## Operational cases reviewed

These responses can represent a dependency outage or an execution failure.
A caller-facing refusal would need a distinct cause at the source.

| Location | Result may depend on |
| --- | --- |
| [api/plugins.rs:50](../../crates/server/src/api/plugins.rs#L50) | Worker cancellation, shutdown, or lost execution |
| [api/dispatch.rs:513](../../crates/server/src/api/dispatch.rs#L513) | Bundle availability and verification |
| [api/sharing/guest.rs:72](../../crates/server/src/api/sharing/guest.rs#L72) | Guest document load or storage failure, with guest limits handled separately |
| [requests.rs:61](../../crates/server/src/requests.rs#L61), [requests.rs:89](../../crates/server/src/requests.rs#L89) | Machine service availability |
| [provider_sign_in/mod.rs:298](../../crates/server/src/provider_sign_in/mod.rs#L298) | Installed provider registration |
| [machines/workers.rs:129](../../crates/server/src/machines/workers.rs#L129), [machines/workers.rs:243](../../crates/server/src/machines/workers.rs#L243), [machines/workers.rs:246](../../crates/server/src/machines/workers.rs#L246) | Lease readiness, missing pool enrollment, or disconnected pool |
| [machines/admission.rs:230](../../crates/server/src/machines/admission.rs#L230), [machines/admission.rs:303](../../crates/server/src/machines/admission.rs#L303) | Sandbox availability and machine placement |
| [turns/run.rs:158](../../crates/server/src/turns/run.rs#L158), [turns/run.rs:164](../../crates/server/src/turns/run.rs#L164), [turns/run.rs:248](../../crates/server/src/turns/run.rs#L248) | Installed provider, adapter, or qualified harness availability |
| [turns/mod.rs:644](../../crates/server/src/turns/mod.rs#L644) | Installed plugin availability |

## Simplification opportunities

Separate worker cancellation and admission decisions from transport failures
where the worker outcome is produced.
