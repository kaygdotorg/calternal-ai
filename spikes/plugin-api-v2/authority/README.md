# Sealed host fixture

Start on dev-01 from the lane's existing source copy.

```sh
cd ~/Developer/lanes/plugin-api-v2-step0
nix develop -c cargo test --locked --release -p calternal-server \
  --test v2_step0_authority -- --ignored --exact serve_authority_fixture --nocapture
```

The launcher prints the loopback URL and the paths to its two disposable
bearer files. It creates fresh roots at each launch and removes them at
normal shutdown. Use `STEP0_AUTHORITY_PORT` to choose another port. A
renderer on another machine can forward the default port with
`ssh -N -L 43125:127.0.0.1:43125 claude@calternal-dev-01-compute02`.

The default base is `http://127.0.0.1:43125/step0/v2`. Each request sends
`Authorization: Bearer <fixture bearer>`. Local browser origins can use
the same endpoint through its CORS preflight.

1. `GET /fixture` returns the manifest, visible `views.home` and
   `views.shared` refs, and a per-view template under `templates`.
   The top-level `template` is the member template.
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

The fixture serves the home and shared Notes views. The owner can edit both. The second
account initially reads the shared view as a member. The scripted
integration target also exercises item-only sharing and revocation.

```sh
nix develop -c cargo test --locked --release -p calternal-server \
  --test v2_step0_authority -- --ignored sealed_ --nocapture --test-threads=1
```

Routes, references and activation contexts belong to this fixture only.
The listener binds to loopback and uses fresh sample Notes and collections.
