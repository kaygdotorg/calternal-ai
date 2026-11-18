---
title: "sharing.set_link_policy"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Set the authority needed to manage public links

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "widens_access",
  "id": "sharing.set_link_policy",
  "description": "Set the authority needed to manage public links",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Set who can manage item links in a space.",
  "properties": {
    "editors_can_manage_links": {
      "description": "Whether the space Editor capability can manage links.",
      "type": "boolean"
    },
    "space_id": {
      "description": "The space whose link policy changes.",
      "type": "string"
    }
  },
  "required": [
    "editors_can_manage_links",
    "space_id"
  ],
  "title": "ShareLinkPolicyInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": true,
  "title": "Map_of_AnyValue",
  "type": "object"
}
```
