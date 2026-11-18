---
title: "onboarding.state"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read your onboarding status

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "onboarding.state",
  "description": "Read your onboarding status",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Both operations derive their account from the authenticated session.",
  "title": "OnboardingInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Durable completion status. Accounts without a completion record start unfinished.",
  "properties": {
    "finished": {
      "description": "True once this account has finished onboarding.",
      "type": "boolean"
    }
  },
  "required": [
    "finished"
  ],
  "title": "OnboardingState",
  "type": "object"
}
```
