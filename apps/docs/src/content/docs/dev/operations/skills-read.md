---
title: "skills.read"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read the portable document of a named skill

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "target": "space",
  "id": "skills.read",
  "description": "Read the portable document of a named skill",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "An operation selecting the named skill in a space.",
  "properties": {
    "name": {
      "description": "Name of the bundle directory.",
      "type": "string"
    },
    "space_id": {
      "description": "Owning space UUID.",
      "type": "string"
    }
  },
  "required": [
    "name",
    "space_id"
  ],
  "title": "SkillTarget",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "SkillDocument": {
      "description": "Descriptor and exact Markdown for one skill.",
      "properties": {
        "agents": {
          "default": [],
          "description": "Harness labels selecting the skill. Empty means all eligible agents.",
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "description": {
          "description": "One-line description declared in the file.",
          "type": "string"
        },
        "id": {
          "description": "Stable content item for an application-managed skill.",
          "type": [
            "string",
            "null"
          ]
        },
        "markdown": {
          "description": "Standard portable Markdown contents.",
          "type": "string"
        },
        "name": {
          "description": "Skill directory name.",
          "type": "string"
        },
        "path": {
          "default": "",
          "description": "Path to SKILL.md relative to the space.",
          "type": "string"
        },
        "revision": {
          "description": "Opaque version to send on update or remove.",
          "type": "string"
        }
      },
      "required": [
        "description",
        "markdown",
        "name",
        "revision"
      ],
      "type": "object"
    }
  },
  "description": "A single skill read or mutation result.",
  "properties": {
    "skill": {
      "allOf": [
        {
          "$ref": "#/definitions/SkillDocument"
        }
      ],
      "description": "Current skill contents."
    }
  },
  "required": [
    "skill"
  ],
  "title": "SkillResult",
  "type": "object"
}
```
