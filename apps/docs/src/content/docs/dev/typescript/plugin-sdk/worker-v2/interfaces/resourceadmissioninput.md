---
title: "plugin-sdk: worker-v2/interfaces/ResourceAdmissionInput"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / ResourceAdmissionInput

# Interface: ResourceAdmissionInput

Input to a read-only block hook declared for `resources.admit`.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="account_id"></a> `account_id` | `string` | Authenticated sender UUID. |
| <a id="entitlement_groups"></a> `entitlement_groups` | `string`[] | Groups available to an operator-approved entitlement policy. |
| <a id="guests"></a> `guests` | `object`[] | Observed sandbox windows for the authenticated sender and scope. |
| <a id="machine_class"></a> `machine_class` | `"hosted"` | Hosted placement being considered. |
| <a id="request_id"></a> `request_id` | `string` | Stable lease request UUID. |
| <a id="runtime_ns"></a> `runtime_ns` | `string` | Confirmed nanoseconds in the retained observation interval. |
| <a id="scope"></a> `scope` | [`ResourceOwner`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/resourceowner/) \| `null` | Resource owner derived by the host. Null denotes an unassigned space. |
| <a id="space_id"></a> `space_id` | `string` | Space UUID assigned by the host. |
| <a id="storage"></a> `storage` | [`SpaceUsage`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/spaceusage/) | Logical bytes committed in this space. |
| <a id="unresolved_generations"></a> `unresolved_generations` | `number` | Number of unresolved generation ends. |
