---
title: "calternal search manifests"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/search-manifests.txt, crates/cli/tests/golden/search-manifests.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `search manifests --space UUID` | Read enabled plugin palette contributions. | Plugin, command and Settings tables. | `manifests` |

## JSON data shape

```json
{
  "manifests": [
    {
      "plugin": "string",
      "version": "string",
      "commands": [
        {
          "id": "string",
          "label": "string",
          "group": "string",
          "words?": [
            "string"
          ],
          "op?": "string",
          "go?": "string"
        }
      ],
      "settings_sections": [
        {
          "id": "string",
          "label": "string",
          "go": "string"
        }
      ]
    }
  ]
}
```

## Human example

Recorded test fixture:

```text
Plugin:   calternal-notes
Version:  0.1.0
COMMAND   ID         GROUP  ACTION    PHRASES
New note  notes.new  Notes  note.new  create note
SETTINGS  ID             ADDRESS
Display   notes.display  /settings/notes.display
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "manifests": [
      {
        "commands": [
          {
            "group": "Notes",
            "id": "notes.new",
            "label": "New note",
            "op": "note.new",
            "words": [
              "create note"
            ]
          }
        ],
        "plugin": "calternal-notes",
        "settings_sections": [
          {
            "go": "/settings/notes.display",
            "id": "notes.display",
            "label": "Display"
          }
        ],
        "version": "0.1.0"
      }
    ]
  },
  "ok": true,
  "request_id": "req-search"
}
```
