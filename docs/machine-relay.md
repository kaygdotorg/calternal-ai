# Machine previews

The machine relay lets a person publish a dev server already listening in
their thread's sandbox. The Calternal server returns a bounded HTTP preview
through the machine's existing outbound connection.

## Design

Reuse the machine session, sequenced `SyncResponse` outbox, lease fence,
`OutputPost` acknowledgements, and the existing sandbox exec output buffer.
A preview fetch is a distinct machine message. The Podman sandbox checks its
own network mode and runs a bounded HTTP reader inside the container. It
dials only IPv4 loopback on the published port. The sandbox needs Node.js.
Other backends cannot reach a preview, including direct execution, which
would share the machine's host loopback.

The run's account can publish when it has `plugins.use` in the thread's
space. Signed-in members with `space.read` can view. The server checks current
space access and the lease fence before returning bytes. Unknown, foreign,
expired and ended previews return the same 404. A preview is transient and
ends when its lease closes, its fence changes, or its 15 minutes expire.

The server allows four publications per lease and 128 per process. It allows
four concurrent fetches per publication and 32 per process. A fetch has
a 2 KiB path and query limit, an 8-second sandbox deadline, and a 1 MiB body
limit. Fetches carry no caller headers or request body, and redirects are
refused. The server returns only a safe content type and a sandboxed document
policy. This first preview supports simple HTTP reads and self-contained
HTML. WebSocket upgrades and browser network access are disabled.

## API

Publish from an active run with
`POST /api/v1/machines/runs/{lease}/previews` and a body such as
`{"port":3000}`. The response contains a preview `id` and its root `path`.
A signed-in viewer can GET that path or append a path and query for the
same local service. The dev server should listen on `127.0.0.1` inside the
container. Stop the run to close its previews immediately.
