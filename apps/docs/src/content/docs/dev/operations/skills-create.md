---
title: "skills.create"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Create a portable skill in its own space folder

Capability: content.write.

## Declaration

```json
{
  "risk": "reversible",
  "required_capability": "content.write",
  "target": "space",
  "id": "skills.create",
  "description": "Create a portable skill in its own space folder",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "The entire portable SKILL.md document to write.",
  "properties": {
    "base_revision": {
      "description": "Required for update and omitted for create.",
      "type": [
        "string",
        "null"
      ]
    },
    "markdown": {
      "description": "Standard YAML frontmatter followed by Markdown instructions.",
      "type": "string"
    },
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
    "markdown",
    "name",
    "space_id"
  ],
  "title": "SkillWrite",
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
