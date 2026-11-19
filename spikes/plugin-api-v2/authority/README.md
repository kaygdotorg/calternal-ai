# Sealed host fixture

Start on dev-01 from the lane's existing source copy.

```sh
cd ~/Developer/lanes/plugin-api-v2-step0
nix develop -c cargo test --locked --release -p calternal-server \
  --test v2_step0_authority -- --ignored --exact serve_authority_fixture --nocapture
```

For 5,000 Notes rows and 5,000 root Threads rows, set
`STEP0_AUTHORITY_SCALE=1` before running the same command. The default
app views have 12 rows. Set `STEP0_AUTHORITY_CHURN_HZ=3` for three
mutations per second in each scale list. Churn defaults to zero and is
bounded to 0–10 Hz. It cycles create, rename and trash. The fixture
tests renderer behavior over in-memory rows.

The launcher prints the loopback URL and the paths to its two disposable
bearer files. It creates fresh roots at each launch and removes them at
normal shutdown. Use `STEP0_AUTHORITY_PORT` to choose another port. A
renderer on another machine can forward the default port with
`ssh -N -L 43125:127.0.0.1:43125 claude@calternal-dev-01-compute02`.

The default base is `http://127.0.0.1:43125/step0/v2`. Each request sends
`Authorization: Bearer <fixture bearer>`. Local browser origins can use
the same endpoint through its CORS preflight.

1. `GET /fixture` returns view refs and per-view sealed templates under
   `templates`. `views.notes`, `views.threads` and `views.reading` are the
   app views with chrome. Their manifests are under `manifests`. The
   existing `home` and `shared` views still exercise the authority matrix.
   The app templates, manifests and wire validator pin
   [revision 13.18](https://git.kayg.org/kayg/calternal-ai/src/commit/195935f4dbed93d660f79ed4bde27c00fffd550f/contract/fixtures/description-v2).
   The Notes toolbar mounts the pinned bulk action for scripted use. The
   Home content list has the sealed `ListNode.selectionMode: multiple`
   declaration and shows the tick controls. The Notes sidebar is navigation.
   Notes projects snippets for secondary text and the row-image fallback.
   Every third initial Notes row includes a sample thumbnail when the
   source revision is current. Fetch it from the view host at
   `GET /api/v2/row-images/{ref}` with the bearer and lease header.
   Once the source head changes the old ref returns an opaque 404.
   The top-level `template` is the original small member template.
2. `POST /view.open` takes `{"type":"view_open","view_ref":"…"}`. Keep its
   lease, grant revision and cache partition.
3. `POST /window` takes the sealed event
   `{"type":"window","request":{"bind":"notes","from":0,"to":20}}`.
   Supply the issued lease ref in the `X-Step0-View-Lease` header. The
   same header supplies view scope to `POST /selection.settled`, whose body
   is `{"type":"selection","bind":"notes","row":"<issued ID>"}`. Replies are
   bare catalogue objects. A window plan refusal uses HTTP 422 with the
   sealed `WindowPlanError`. Other refusals use an HTTP error and an
   `{"error":"<code>"}` body.
4. The action endpoints are `POST /activation.resolve`,
   `/activation.confirm`, `/activation.dispatch`, `/choice.options`
   and `/view.renew`. Send the matching sealed event body. Pin’s returned
   Undo uses `POST /undo` and the current lease header.

Notes uses `notes`, `collections`, `trash`, `trash-count`, `note`,
`current` and `note-search`. The fixture transport takes
`X-Step0-Route-Collection` and `X-Step0-Route-Item` on window requests
for the selected collection or note. To settle a search, send
`POST /search` with the lease header and
`{"type":"search","bind":"note-search","text":"Note"}`, then request that
Bind's window. Use optional `match: "terms"` or `match: "prefix"`.
The Bind owns the field filter. Reading List uses the same flow with
`reading-search`.

Threads uses `waiting` for the Inbox, `all` for root rows, `children` for
each parent's nested window, `layout` for the viewer's two sections, and
`in-section` for each section's placements. Use the matching
`rows[].parent_refs` entry as `request.parent`. Requests across parents
or edges use the usual lease and ref checks.

Read `WindowResult.activation_contexts` by row ID and action ID. Use the
view map for New note, New thread, New section and Notes Trash selection.
The scale rows support Pin with Undo, Rename, Move to collection and
Trash/Restore. Threads support Archive, personal section actions and
moves using the checked ChoiceRef for the target section. Resolve a
bulk Trash with up to 50 IDs from issued Notes windows in
`interaction.rows`, or call `POST /selection.set` with
`{"type":"selection_set","bind":"notes","rows":[...]}` and the lease header.
The route works on Home, Shared and Notes for an editor using rows issued
by that view lease. Use its returned `selection_ref` in the activation interaction. Data changes
appear on a fresh window read. These display effects use disposable
in-memory state. The original authority tests still use the real engines.

The owner can submit Reading List's `add` form. Resolve with
`action_id:"add"`, its view-level activation context and
`interaction.form` containing `title` and `url`. Dispatch its resolution on
the normal dispatch route. The outcome carries `created`, and a new
`items` window includes the saved fixture row. The fixture stores the
address locally and does not visit it. Original authority actions remain
available on the small views. The second account reads shared views as
a member by default.

```sh
nix develop -c cargo test --locked --release -p calternal-server \
  --test v2_step0_authority -- --ignored sealed_ --nocapture --test-threads=1
```

The transcript, request-card and composer components in the Threads template
are for drawing. Revision 13.15 gives the composer host-owned Send/Stop and
view contexts but leaves the action ID lookup unspecified. It defines
`RequestCardResolve` but leaves the read route and record unspecified. The
fixture supplies no transcript, composer or request-card routes.

Routes, references and activation contexts belong to this fixture only.
The listener binds to loopback and uses fresh sample Notes and collections.
