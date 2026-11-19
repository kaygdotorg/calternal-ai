# Independent runner paper run · 2026-11-20

Lantern is a community package with its own `lantern-conversation` kind and a
distinct `lantern-model` contribution. The hostname and assets below are
illustrative. This is a paper exercise against revision 13.7 and the
[sealed generic engines proposal](plugin-api-v2-generic-engines.md). It uses
no first-party provider registration, subscription credential, or Turns
capability. The conversation belongs in Lantern's app. It can use the same
host Inbox and transcript component as any other conversation kind.

## Manifest and description

The abbreviated declarations below omit asset digests and repeatable
operation metadata. The worker owns the Send policy and replayable job.
The host owns the item, initial message, transitions, and cold projections.

```json
{
  "api_version": 2,
  "id": "lantern",
  "name": "Lantern",
  "version": "0.1.0",
  "icon": "puzzle",
  "apps": [{"id": "lantern", "name": "Lantern", "icon": "puzzle", "description": {"id": "main", "template": {"path": "main.json", "sha256": "<asset digest>"}}}],
  "reads": ["lantern-conversation"],
  "network": {"fixed_sites": ["inference.lantern.example"]},
  "kinds": [{
    "id": "lantern-conversation",
    "icon": "puzzle",
    "title_field": "name",
    "storage": "ssd",
    "claimed_text": true,
    "searchable": true,
    "conversation": true,
    "schema": {"type": "object", "required": ["name"], "additionalProperties": false, "properties": {"name": {"type": "string"}}},
    "host_create": {"defaults": {"name": "New conversation"}},
    "composer": {"send": "lantern.send", "steer_live": true}
  }],
  "providers": [{
    "id": "lantern-model",
    "name": "Lantern Model",
    "adapter": {"path": "lantern-adapter", "sha256": "<asset digest>"},
    "image": "sha256:<image digest>",
    "models": [{"id": "lantern-small", "label": "Lantern Small"}]
  }],
  "jobs": [{"id": "run", "run_operation": "lantern.run", "replayable": true, "waits": [{"kind": "execution"}]}]
}
```

| Worker operation | Declared target and broker methods |
| --- | --- |
| `lantern.send` | One `ItemRef` of `lantern-conversation`, plus host-validated message material. Host persists the message and enqueue intent before worker execution. The checked intent starts the replayable `run` job. |
| `lantern.run` | Same item and admitted intent. `execution_start`, `execution_status`, `checkpoint_job`. This also handles a bounded child request with `execution_spawn_child`; the child kind and default fields come from Lantern's declarations. |

A list Bind shows title and state. Its row declares `host.item.archive`,
`host.item.unarchive` and `host.item.snooze` through the normal activation
context. The composer and transcript bind the same checked conversation:

```json
[
  {"type":"conversation-transcript","id":"messages","config":{"conversation":{"route":"item"},"follow":"end","quote":true}},
  {"type":"composer","id":"message","config":{"conversation":{"route":"item"},"controls":["provider","model","machine","stop"]}}
]
```

The Inbox's `request-card` takes a checked RequestRef. A list can expose
`waiting_on` only for the viewer's own requests. The description cannot
author approval answers, print provider credentials, or infer another
viewer’s Inbox state.

## Adapter

The pinned executable runs in the agent VM. It receives checked inputs and
an attempt-bound broker channel. The machine runtime owns the approved
provider transport and injects its credential on the approved destination.
The adapter requests inference using a handle, emits bounded
`provider-events-v1`-based display frames, and advertises input support.
It translates steer to the active provider stream and handles messages from
the approved tool gateway. It has no provider home mount or raw bearer.

A host qualification service runs the same automated suite for every digest
and placement. A paper executable and an illustrative hostname cannot
qualify. The credential follow-up remains a separate real-provider gate.
[Credential measurements](../measurements/provider-credential-brokering-2026-11-20.md)

## Call sequence and blockers

| Step | Sealed or proposed surface | What the package can express |
| --- | --- | --- |
| Create | 13.7 `host.item.create` with manifest `host_create` | Creates an empty package-owned item cold. The proposed `conversation: true` enables the transcript, corpus and checked execution relation. 13.7 supplies those only to the host Threads kind. |
| Prepare and send | 13.7 composer options and activation exchange. Proposed declared Send and cold receipt-backed intent. | Host resolves provider, model, placement and attachments before Send. Final text is content. A changed command or mention needs a fresh preview. Today the special host Send dispatch is hardwired to Turns. |
| Admit and run | Proposed `execution_start` from a replayable job. | Use the receipt and expected item head to admit once under live actor rights. Machine supervisor starts the pinned image on the space’s agent VM. Provider login and transport qualification are host preconditions. |
| Stream and park | Proposed common display envelope and `execution` wait, plus `transcript.page` retargeted from ThreadRef to conversation ItemRef. | Host commits a canonical Markdown record and private spool receipt separately. The package parks against a durable transition cursor and stays asleep on tokens. 13.7 has no execution wait or generic transcript. |
| Approval | 13.7 protected `request_resolve`, extended to the live execution. | Native actions proven to remain inside that attempt can be answered for its exact process and request. For broker effects the gateway consumes a one-use grant against verified operation material. A native request with external effect stays dismissal-only. |
| Steer | Proposed `execution_input`. | Prepared Send with `steer_live` forwards host-checked text to the pinned active attempt without waking the plugin. A changed attempt requires fresh confirmation. |
| Child | Proposed `execution_spawn_child`, issued by the package job. | The agent requests a child. The host wakes the declared parent job with checked material. It commits generic child create and the parent edge under the admission fence, then starts the child. Its refs intersect the request, live caller and every ancestor. 13.7 has no child admission input or tree quota. |
| Stop | Proposed `host.execution.stop` and `execution_stop`. | Host checks exact live attempt and starter/Manager rights, fences immediately and sends Stop. A generic item grant gives no stop right. |
| Archive and Undo | Proposed generic archive/unarchive, existing Undo surface. | Idle archive returns one atomic Undo only after the writer qualifies. An active attempt needs a challenge bound to the exact attempt and live stop rights. Undo restores the archived head and any retired grants only when they committed together. |
| Restart | Proposed runtime deadlines and same-execution proof, record-level page cursor. | Runtime freezes on the earlier cutoff. An intact attempt with safe transport can thaw after fresh renewal. Unknown outcomes require explicit Resume. Host reopens checked records and refreshes the notice prefix in one reply. Exact replay preserves durable receipts. |

**Still needs implementation proof.** There is no executable draft in this
document. The key seams are the atomic cold Send and intent, generic child
create and admission, the transport-owned credential and runtime proof,
park-versus-transition ordering, record recovery, and the archive/Undo
writer. Until those gates pass, leave the legacy engine in place. Keep
refs-only discovery as the interim policy, pending owner approval.
