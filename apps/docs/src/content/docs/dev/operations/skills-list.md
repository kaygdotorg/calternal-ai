---
title: "skills.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List visible space skills and their descriptions.

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "target": "space",
  "id": "skills.list",
  "description": "List visible space skills and their descriptions.",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Picker filter within one space.",
  "properties": {
    "prefix": {
      "default": "",
      "description": "Literal display-name prefix. Omit to browse.",
      "type": "string"
    },
    "space_id": {
      "description": "Selected space UUID.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceLookupInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "SkillMatch": {
      "description": "One externally placed space skill.",
      "properties": {
        "description": {
          "description": "One-line description declared in the file.",
          "type": "string"
        },
        "name": {
          "description": "Skill directory name.",
          "type": "string"
        }
      },
      "required": [
        "description",
        "name"
      ],
      "type": "object"
    }
  },
  "description": "Bounded skill matches for the selected space.",
  "properties": {
    "skills": {
      "description": "Skill descriptors, ordered by name.",
      "items": {
        "$ref": "#/definitions/SkillMatch"
      },
      "type": "array"
    }
  },
  "required": [
    "skills"
  ],
  "title": "SkillsResult",
  "type": "object"
}
```
