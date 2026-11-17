# Addresses

Decided by kayg on 2026-11-17. For Calternal-AI this supersedes the existing app's ADR 0056
(`docs/adr/0056-plain-addresses-and-stable-block-ids.md` in `/home/kayg/Developer/calternal`).
Its block-identity rules (B1 to B5) still apply; its address shapes, readable suffixes and
legacy aliases do not.

## Rule

Every address is `/<kind>/<id>`, optionally followed by something inside that thing.
Paths are plain (no `#/`), rooted at `/`.

| Thing | Address |
| --- | --- |
| An item | `/<kind>/<item-id>` such as `/notes/<id>`, `/tasks/<id>`, `/files/<id>`, `/mail/<id>`, `/chats/<id>` |
| A block in an item | `/<kind>/<item-id>/<block-id>` |
| A space | `/spaces/<space-id>` |
| A list or view inside a space | `/spaces/<space-id>/<kind>[/<view>...]` such as `/spaces/<id>/notes`, `/spaces/<id>/notes/tags/travel` |
| Space administration | `/spaces/<space-id>/members`, `/spaces/<space-id>/plugins/<plugin-id>` |
| A person | `/people/<account-id>` |
| A setting | `/settings/<setting-id>` |
| An invite | `/invites/<secret>` |
| Cross-space views | `/activity` |

## Rules that follow

- `<kind>` is the kind of thing, not the ID of the plugin that opens it. Replacing a plugin
  does not change any address.
- An item's address never contains its space. The item ID is globally unique; the server
  finds the owning space from it and checks the caller's current access. A wrong kind for
  an existing ID is not found.
- `/spaces/<space-id>/<kind>` is only for a collection inside one space. There is no
  `/spaces/<space-id>/<kind>/<item-id>` form; one thing has exactly one address.
- IDs are UUIDs written in full, the same in files, API and URLs. They are never derived
  from a title or path. Accounts, spaces and items each have their own; none is shared.
- There is no readable-name suffix. Clients show a title by looking the ID up.
- Possessing an address grants nothing. A caller without access gets the same not-found
  answer as for an ID that does not exist.
- No legacy forms are supported: no `#/` conversion, six-character block IDs,
  `day-YYYY-MM-DD` items or old-route aliases.
- A deleted item's ID keeps resolving to its Trash or deleted state and is never reused.
