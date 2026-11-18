---
title: "jobs.get"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read a plugin job's durable record

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "jobs.get",
  "description": "Read a plugin job's durable record",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for `jobs.get` and `jobs.cancel`. Possessing a job ID grants nothing: a job in a space the caller cannot read answers exactly like a missing one.",
  "properties": {
    "job_id": {
      "description": "Host-issued job UUID.",
      "type": "string"
    }
  },
  "required": [
    "job_id"
  ],
  "title": "JobInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "JobState": {
      "description": "Lifecycle of a job. `admitted`, `running` and `waiting` are active; the rest are final and never change again.",
      "oneOf": [
        {
          "description": "Durably recorded and queued for a run.",
          "enum": [
            "admitted"
          ],
          "type": "string"
        },
        {
          "description": "A run invocation is executing in the plugin's worker.",
          "enum": [
            "running"
          ],
          "type": "string"
        },
        {
          "description": "Parked without a process until its child jobs report, a signal arrives or a deadline passes.",
          "enum": [
            "waiting"
          ],
          "type": "string"
        },
        {
          "description": "Machine or server went silent; only explicit resume may restart this turn.",
          "enum": [
            "interrupted"
          ],
          "type": "string"
        },
        {
          "description": "No provider process is held; checkpoint.retry records the durable next attempt.",
          "enum": [
            "retry_waiting"
          ],
          "type": "string"
        },
        {
          "description": "Finished with a result.",
          "enum": [
            "done"
          ],
          "type": "string"
        },
        {
          "description": "Finished without a result.",
          "enum": [
            "failed"
          ],
          "type": "string"
        },
        {
          "description": "Cancelled; later output from its run is discarded.",
          "enum": [
            "cancelled"
          ],
          "type": "string"
        },
        {
          "description": "Interrupted by a server restart while its effects could not be safely repeated.",
          "enum": [
            "outcome_unknown"
          ],
          "type": "string"
        }
      ]
    },
    "JobView": {
      "additionalProperties": false,
      "description": "What a client may see of a job: its durable record, never its stream.",
      "properties": {
        "checkpoint": {
          "description": "Last durable checkpoint written by the plugin, or null."
        },
        "checkpoint_seq": {
          "description": "Stream sequence number of the first chunk after that checkpoint.",
          "format": "uint64",
          "minimum": 0,
          "type": "integer"
        },
        "id": {
          "description": "Host-issued job UUID.",
          "type": "string"
        },
        "parent_id": {
          "description": "Parent job UUID for a child job.",
          "type": [
            "string",
            "null"
          ]
        },
        "plugin_id": {
          "description": "Plugin that runs the job.",
          "type": "string"
        },
        "result": {
          "description": "Final result, a caller-safe wait rejection, or null."
        },
        "space_id": {
          "description": "Space UUID that owns the job.",
          "type": "string"
        },
        "state": {
          "allOf": [
            {
              "$ref": "#/definitions/JobState"
            }
          ],
          "description": "Current lifecycle state."
        },
        "wait": {
          "anyOf": [
            {
              "$ref": "#/definitions/JobWait"
            },
            {
              "type": "null"
            }
          ],
          "description": "Public summary while a plugin job waits for a signal."
        }
      },
      "required": [
        "checkpoint",
        "checkpoint_seq",
        "id",
        "plugin_id",
        "result",
        "space_id",
        "state"
      ],
      "type": "object"
    },
    "JobWait": {
      "additionalProperties": false,
      "description": "Public summary of a parked plugin job with no signal payload or credentials.",
      "properties": {
        "deadline_ms": {
          "description": "Unix milliseconds when the wait ends if no signal arrives.",
          "format": "int64",
          "type": "integer"
        },
        "reason": {
          "description": "Why the job is waiting, supplied by the worker at the checkpoint.",
          "type": "string"
        }
      },
      "required": [
        "deadline_ms",
        "reason"
      ],
      "type": "object"
    }
  },
  "description": "Result for `jobs.get` and `jobs.cancel`.",
  "properties": {
    "job": {
      "allOf": [
        {
          "$ref": "#/definitions/JobView"
        }
      ],
      "description": "The job after the operation."
    }
  },
  "required": [
    "job"
  ],
  "title": "JobResult",
  "type": "object"
}
```
