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
| An item | `/<kind>/<item-id>` such as `/notes/<id>`, `/tasks/<id>`, `/mail/<id>`, `/chats/<id>` |
| A file or a folder | `/files/<id>`; a folder is an item of kind `files` and is never addressed by path |
| A block in an item | `/<kind>/<item-id>/<block-id>` |
| A collection (a list of things) | `/collections/<collection-id>` |
| A space | `/spaces/<space-id>` |
| Space administration | `/spaces/<space-id>/members`, `/spaces/<space-id>/plugins/<plugin-id>`, `/spaces/<space-id>/settings` |
| A person | `/people/<account-id>` |
| A setting | `/settings/<setting-id>` |
| An invite | `/invites/<secret>` |
| Your activity | `/activity` |

## Rules that follow

- `<kind>` is the kind of thing, not the ID of the plugin that opens it. Replacing a plugin
  does not change any address.
- An item's address never contains its space. The item ID is globally unique; the server
  finds the owning space from it and checks the caller's current access. A wrong kind for
  an existing ID is not found.
- A collection is a stored thing with its own ID, a kind and an owner. It is owned by a
  space (that space's notes, its Trash, its Sent mailbox, a tag's list, a saved view) or by
  an account (a list across all of that person's spaces). A plugin's standard collections
  are created with their IDs when the plugin is enabled in a space, so every list has an
  address from the start. There is no `/spaces/<space-id>/<kind>` form.
- Opening a collection opens the app that shows it. Opening one of its rows goes to that
  item's own address.
- Transient parameters that are not things (a calendar date, a search query, a sort order)
  go in the query string of the collection's address and never identify anything.
- A tag is named in files by its text; its list is a collection, so the list's address
  survives a rename.
- IDs are UUIDs written in full, the same in files, API and URLs. They are never derived
  from a title or path. Accounts, spaces and items each have their own; none is shared.
- There is no readable-name suffix. Clients show a title by looking the ID up.
- Possessing an address grants nothing. A caller without access gets the same not-found
  answer as for an ID that does not exist.
- No legacy forms are supported: no `#/` conversion, six-character block IDs,
  `day-YYYY-MM-DD` items or old-route aliases.
- A deleted item's ID keeps resolving to its Trash or deleted state and is never reused.
