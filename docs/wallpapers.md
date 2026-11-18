# Built-in wallpapers through the server

The server serves the eight [built-in wallpapers](settings.md#default-wallpapers)
from its own origin. Set `CALTERNAL_UNSPLASH_KEY` in the environment the
server container loads at startup. It is sent as a sensitive authorization
header to `api.unsplash.com`. The image host receives no authorization header.
With a missing or blank key, valid rendition requests return `404`,
`not_found`, with the message “Unsplash wallpapers are not configured.”
Use the bundled appearance fallback on a failed image request.

## Client contract

Call `wallpapers.list` through `POST /api/v1/operations` with an empty
`input` and a signed-in session. The reply's `data` has `default_id` and
`wallpapers`. Each entry carries `id`, `author`, `author_url` and `link`.
Credit the photographer and Unsplash using the supplied profile and photo
links, which already carry referral parameters. The catalog remains available
without a configured key. The CLI can read it with
`calternal invoke wallpapers.list --input '{}'` or the same command with
`--json`.

Use `GET /api/v1/wallpapers/<id>/<rendition>` for the JPEG. The width variants
retain the shell's 96 px first-paint source and provide denser assets for
`image-set` at 2x and 3x. The shell's viewport-sized copies still use its
existing one-pixel-per-four-device-pixels calculation.

| Rendition | Target width | Access |
| --- | ---: | --- |
| `full` and `light` | 1600 px | Signed in |
| `thumb` | 360 px | Signed in |
| `blur` and `blur-light` | 96 px | Default photo, including sign-in |
| `blur-2x` and `blur-light-2x` | 192 px | Default photo, including sign-in |
| `blur-3x` and `blur-light-3x` | 288 px | Default photo, including sign-in |

Successful responses carry a strong content ETag and 30-day freshness.
Only the sign-in copies may be kept by a shared cache. The private responses
vary on Cookie and Authorization. The same-origin resource and content
security policies still apply. Authorization runs before image delivery and
before returning a private `304`. No space or URL is accepted as input.

## Outbound requests and storage

On a cold request, the server reads photo metadata from `api.unsplash.com`
and uses its `urls.raw` URL on `images.unsplash.com` with the API's `ixid`.
The image host applies the existing shell's resize and light treatments.
As in the original app's backdrop integration, the download event uses the
API's `links.download_location` with authorization, separately from fetching
the image. This lane reports the first wallpaper use of each photo on an
installation, before its first non-thumbnail image is fetched. Thumbnails
alone do not trigger tracking. A disk marker preserves that event across
normal restarts. An interruption after Unsplash accepts the event but before
the marker is durable can repeat the event on retry. The [Unsplash download
tracking guideline](https://help.unsplash.com/api-guidelines/guideline-triggering-a-download)
describes tracking user selection. An explicit picker event would also cover
later selections served from the browser cache.

The ported netguard denies reserved and non-public DNS answers and connects
using only checked addresses. The outbound client uses HTTPS on the two
declared hosts, with proxies and redirects disabled. The shared HTTPS
transport also lets the plugin network broker authorize and validate each
redirect before dialing it. The bounded body reader handles both declared
and streamed lengths. The wallpaper service
accepts at most 48 upstream attempts in any rolling hour of a server
process, including failed requests to either host. A restart resets that
in-memory budget. It handles up to 16 image requests at a time.

The dedicated `appdata/wallpapers` cache retains one bounded file per
allowed rendition and one small tracking marker per photo. Published images
total at most 34.75 MiB across 30 slots. Up to eight in-progress writes add at
most 16 MiB of staging data. Startup evicts obsolete and oversize files,
symlinks and abandoned staging files. Current defaults stay cached. Successful
requests for a published rendition read the cache without going upstream. Cache entries use
owner-only permissions when written and are published with the private
atomic replacement helper.
Remove this directory while the server is stopped if the cache needs resetting.

## Production question

[Unsplash's API guidelines](https://help.unsplash.com/en/articles/2511245-unsplash-api-guidelines)
require attribution, protected API credentials, download tracking and
hotlinking for photo views. Their [hotlinking guidance](https://help.unsplash.com/api-guidelines/guideline-hotlinking-images)
describes a possible view beacon for apps that need to serve images through
their own infrastructure. The same-origin cache preserves `ixid` on the
upstream image request, but cache hits do not generate upstream views.
Confirm a provider-approved same-origin arrangement before rolling out the
cached API integration. Hosting the fixed images under a separately approved
arrangement is an alternative and would require its own asset and attribution
workflow.
