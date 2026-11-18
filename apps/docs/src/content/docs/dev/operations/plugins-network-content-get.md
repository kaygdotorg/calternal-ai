---
title: "plugins.network.content.get"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Get a prepared sealed content frame

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "plugins.network.content.get",
  "description": "Get a prepared sealed content frame",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Read a prepared sealed frame as the person it was created for.",
  "properties": {
    "frame_id": {
      "description": "Short-lived frame ID from content preparation.",
      "type": "string"
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    }
  },
  "required": [
    "frame_id",
    "space_id"
  ],
  "title": "ContentFrameAddress",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "ContentFetchPreference": {
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
      ]
    }
  },
  "description": "Trusted client data for mounting a sealed frame and for external navigation.",
  "properties": {
    "frame_url": {
      "description": "Relative trusted frame URL. Mount with an empty iframe sandbox.",
      "type": "string"
    },
    "links": {
      "description": "External targets in the order of the frame's navigation references. Open only through the host's explicit navigation action.",
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "preference": {
      "allOf": [
        {
          "$ref": "#/definitions/ContentFetchPreference"
        }
      ],
      "description": "Current remote-content preference for the viewer."
    }
  },
  "required": [
    "frame_url",
    "links",
    "preference"
  ],
  "title": "ContentFrameView",
  "type": "object"
}
```
