# Revision 14.1 renderer gap disposition

This draft follows the native lane's
[23 findings](https://git.kayg.org/kayg/calternal-ai/src/branch/lane/v2-renderer-apple/docs/reviews/v2-renderer-contract-gaps-2026-11-20.md)
and the web lane's composer and request-card questions. The detailed wire
lives in [description §5.1](../description-contract.md#51-host-surfaces-and-navigation-on-the-wire)
and the [catalogue](../../contract/host-catalogue-v2.json). The draft is based on the sealed 13.18 revision.

| Finding | Draft disposition |
| --- | --- |
| 1. Route values | Checked bounded navigation refs on open, windows and selections. Activate by context. |
| 2. Transcript records | Closed canonical record with text and role, distinct from provider producer frames. |
| 3. Older paging | Already in the draft. Newest by default, before and after cursors, 60 by default and up to 128. |
| 4. Streaming | Typed retained-page upserts and one provisional preview on the existing subscription. |
| 5. Send and Stop | Host names action IDs by component and issues contexts. Prepared final-text confirmation dispatches Send. |
| 6. Option pages | Catalogue control/option types, upstream dependency order and checked options round trip. |
| 7. Attachments and mentions | Portable upload and lookup stay deferred. Omit their controls unless a host implements the existing checked surface. |
| 8. Quote | Checked committed record and UTF-8 range to typed quote and preview. |
| 9. Request card | Lease-bound record and server-offered decisions. Reuse `RequestCardResolve`. |
| 10. Item editor | Authorised pinned head then reuse range and staged save with revision conflict. Live room stays separate. |
| 11. Sealed frame | Lease-scoped content URL, enforced CSP and byte cap. |
| 12. Image origin | View host, bearer and current lease. Refusal uses the fallback. |
| 13. Kind icons | Optional data from view-open. Missing entry falls back to stock symbol. |
| 14. Viewer identity | Opaque authenticated member ref from view-open. |
| 15. Disabled and roles | Row predicate over authorised projected values; role is the current member role. |
| 16. Timestamps | RFC 3339 and local formatting. Preserve the revision 13.15 fixture corrections when rebasing. |
| 17. Created and then | Write the checked created ref to the named route on success; leave route alone if absent. |
| 18. Cache proof | Domain-separated canonical record signature verified with the authenticated pinned Ed25519 key. |
| 19. Lease renewal | Ignore superseded replies, discard old-grant live state, reopen on lapse. |
| 20. Select All | Explicit selections stay at fifty. Larger sets require the protected operation-specific path. |
| 21. Account footer | Host identity from view-open. Reuse the shell account menu. |
| 22. Search | Typed settle acknowledgment and fresh offset-zero window. Query match is independent of Bind field filter. |
| 23. Counts | Exact as a number, estimates as “about N”, unavailable omitted. |


## Web renderer selection follow-up

| Finding | Disposition |
| --- | --- |
| Notes sidebar has no tick controls | Confirmed contract limit. Multiple selection is declared by a content `ListNode` with `selectionMode: "multiple"`. A sidebar `Section` only consuming the Bind does not supply the selection set. Use the existing content list for ticked bulk actions. In the fixture, Home demonstrates that path. The Notes toolbar's grafted action supports scripted selection tests. |
| `selection.set` returned 404 on Home and 200 on Notes | Fixture routing bug. Home already declares the multiple-selection list and bulk action. Both views use the same typed route. The host checks the current lease and editor capability, then resolves only rows previously issued for that Bind and view. An empty set clears its prior ref. |
