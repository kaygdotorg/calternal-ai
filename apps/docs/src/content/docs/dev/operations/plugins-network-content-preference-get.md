---
title: "plugins.network.content.preference.get"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read your remote content preference

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "plugins.network.content.preference.get",
  "description": "Read your remote content preference",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Read the viewer's remote content policy in one space.",
  "properties": {
    "space_id": {
      "description": "Space UUID. The authenticated caller supplies the account identity.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "ContentFetchPreferenceAddress",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "One person's policy for remote images and fonts in a space.",
  "oneOf": [
    {
      "description": "Request an explicit decision before downloading remote assets.",
      "enum": [
        "ask"
      ],
      "type": "string"
    },
    {
      "description": "Fetch eligible assets at ingest, with the older-content fallback.",
      "enum": [
        "always"
      ],
      "type": "string"
    },
    {
      "description": "Show placeholders for remote assets.",
      "enum": [
        "never"
      ],
      "type": "string"
    }
  ],
  "title": "ContentFetchPreference"
}
```
