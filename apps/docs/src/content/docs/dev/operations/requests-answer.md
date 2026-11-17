---
title: "requests.answer"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Answer a pending question

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "requests.answer",
  "description": "Answer a pending question",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Resolve a pending question with the submitted form values.",
  "properties": {
    "answer": {
      "additionalProperties": true,
      "description": "Submitted form values.",
      "type": "object"
    },
    "id": {
      "description": "Question UUID.",
      "type": "string"
    }
  },
  "required": [
    "answer",
    "id"
  ],
  "title": "RequestAnswerInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "Result of resolving a private request.",
  "properties": {
    "id": {
      "description": "Resolved request UUID.",
      "type": "string"
    },
    "status": {
      "description": "Answered, approved or denied.",
      "type": "string"
    }
  },
  "required": [
    "id",
    "status"
  ],
  "title": "RequestResolution",
  "type": "object"
}
```
