# Files and Photos (mockup lane `mockup-files`)

Fixture-only pages for the two apps, registered in `index.ts`. Nothing here is real: names are invented and every picture is generated (`photos/scene.ts`).

- `files/`: `data.ts` (a tree of about 150 files, a 52-file folder, an empty folder, shared and favourites), `FilesPage` (Home, a folder as list or gallery, sortable columns, breadcrumb, drag-over hint), `FilePreview` (PDF pages, document, image, spreadsheet, slide, audio/video transport, archive, code), `FileThumb` (a picture drawn per file family), `FilesPanel` (details, sharing, keep downloaded; storage and what you share when nothing is open).
- `photos/`: `data.ts` (168 photos over seven weeks, albums, library lists), `PhotosPage` (timeline by day or month, dense square grid, album page), `PhotoViewer` (picture, arrows, thumbnail strip, keys), `PhotosPanel` (date, place with a generated map, camera, people, albums).
- `Look.svelte`: the row, strip, card and cell looks of the `file` and `photo` kinds, via the seam's `looks`.
- Folders and albums are `file` / `photo` items with `folder: true`; they open in place by setting the window's `sel`, which the page reads (no sel is Home or the whole library).
- Capture addresses: `?app=files&second=fo-trip` (folder), `fr-0` (PDF), `fi-plan` (long document), `fr-1` (sheet), `fi-sunset` (image), `fo-scans` (empty); `?app=photos&second=al-lisbon` (album), `ph-20` (a photo), `pl-fav` (favourites). Add `&scheme=dark`, `&phone=1`.
