# Skills and automations backend verification

The backend and UI operation shapes are recorded in [Forgejo #57](https://git.kayg.org/kayg/calternal-ai/issues/57). The final dev-01 gate ran commit `5c1704d4` from `lane/automations`, including the merged `5dea7842` backend baseline and its next documentation change. The build used slot 3, `nix develop`, and six Cargo jobs.

Managed skill updates preserve bundle resources. Removal moves the bundle to Trash. Skills placed outside the application appear in list and read operations. The automation preview returns the original words, normalized calendar expression, confirmed account zone, and next run. The scheduler keeps 128 definitions and 32 queued handoffs per space. It uses the creator's current space authority and existing thread admission, permission modes, and approval requests.

The migration inventory adds `system/automations.db` at baseline version **1**, through the shared runner. The transition test verifies an empty database advances from version 0 to 1 and leaves data intact when reopened. Automation Markdown uses `calternal-automation/v1`. Existing database versions stay fixed in this lane.

| Gate | Passed | Failed | Ignored or skipped |
| --- | ---: | ---: | ---: |
| Rust workspace with all features | 655 | 0 | 2 |
| Plugin tests | 32 | 0 | 1 |
| Integration | 2 | 0 | 0 |
| Serial conformance against the test-plugin server | 140 | 0 | 0 |

Plugin build, both formatting checks, the test-plugin server build, docs site, docs check, and generated contract check exited successfully. The docs site verified 365 source pages. The skips were the binary-dependent Rust smoke test under the workspace invocation, the perf-lock microVM measurement, and the live Claude sign-in opt-in. Build outputs were removed from both dev VMs. The process scan found no remaining processes for this lane's dev-01 checkout.

An earlier conformance run exposed a missing inventory entry for the new operations and an empty result in the existing message-search case. The foreign-space matrix now exercises all 15 new operations. The final full run passed both cases. The latest run also verified manual replay, creator access revocation, time-zone confirmation, and bundle removal. The crash-recovery tests cover durable occurrence identity and replay. Full fault injection at each thread and machine handoff boundary remains unverified.

### Simplification opportunities

Read next and last status in one query when listing definitions. Moving per-space scheduler state into space-owned storage would reduce contention on the shared SQLite writer.
