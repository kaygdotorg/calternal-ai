---
title: "calternal agent readiness"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/agent-readiness.txt, crates/cli/tests/golden/agent-readiness.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `agent readiness --space-id UUID [--thread-id UUID]` | Read cached placement capacity and provider sign-in for the caller. | Adaptive machine and provider table, with the official sign-in command. | `machines[]`, `pool[]`, `check_ttl_ms` |

## JSON data shape

```json
{
  "machines": [
    {
      "machine_id": "string",
      "name": "string",
      "enrolled": "bool",
      "connected": "bool",
      "available": "bool",
      "execution": "string",
      "last_seen_ms": "any",
      "providers": "any"
    }
  ],
  "pool": [
    {
      "enrolled": "bool",
      "connected": "bool",
      "available": "bool",
      "execution": "string",
      "providers": "any"
    }
  ],
  "check_ttl_ms": "integer"
}
```

## Human example

Recorded test fixture:

```text
Machine      Connection  Execution  Capacity     Machine ID                            Last seen                   Provider               Sign-in     Readiness
Development  Connected   sandboxed  Available    019a8c00-0000-7000-8000-000000000001  2025-10-09 08:53:20 +00:00  calternal-claude-code  Signed out  Unavailable
Cloud pool   Offline     sandboxed  Unavailable  —                                     —                           calternal-claude-code  Unknown     Unavailable

Sign-in commands
Development · calternal-claude-code:  claude auth login
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "check_ttl_ms": 300000,
    "machines": [
      {
        "available": true,
        "connected": true,
        "enrolled": true,
        "execution": "sandboxed",
        "last_seen_ms": 1760000000000,
        "machine_id": "019a8c00-0000-7000-8000-000000000001",
        "name": "Development",
        "providers": {
          "calternal-claude-code": {
            "check": {
              "installed": true,
              "sign_in_command": "claude auth login",
              "signed_in": false
            },
            "enabled": true,
            "ready": false
          }
        }
      }
    ],
    "pool": [
      {
        "available": false,
        "connected": false,
        "enrolled": true,
        "execution": "sandboxed",
        "providers": {
          "calternal-claude-code": {
            "check": null,
            "enabled": true,
            "ready": false
          }
        }
      }
    ]
  },
  "ok": true
}
```
