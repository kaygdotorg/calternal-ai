---
title: "wallpapers.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List built-in wallpapers with Unsplash attribution

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "wallpapers.list",
  "description": "List built-in wallpapers with Unsplash attribution",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Empty input for the built-in catalog.",
  "title": "WallpapersListInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "WallpaperCredit": {
      "additionalProperties": false,
      "description": "Photo and photographer attribution with Unsplash referral parameters.",
      "properties": {
        "author": {
          "description": "Photographer's display name.",
          "type": "string"
        },
        "author_url": {
          "description": "Photographer's Unsplash profile URL for the credit tooltip.",
          "type": "string"
        },
        "id": {
          "description": "Unsplash photo ID used in the same-origin rendition route.",
          "type": "string"
        },
        "link": {
          "description": "Photo's Unsplash page URL for attribution to Unsplash.",
          "type": "string"
        }
      },
      "required": [
        "author",
        "author_url",
        "id",
        "link"
      ],
      "type": "object"
    }
  },
  "description": "Fixed photo catalog returned without contacting the image service.",
  "properties": {
    "default_id": {
      "description": "Photo used when no wallpaper was chosen.",
      "type": "string"
    },
    "wallpapers": {
      "description": "All eight built-in photos in picker order.",
      "items": {
        "$ref": "#/definitions/WallpaperCredit"
      },
      "type": "array"
    }
  },
  "required": [
    "default_id",
    "wallpapers"
  ],
  "title": "WallpapersListResult",
  "type": "object"
}
```
