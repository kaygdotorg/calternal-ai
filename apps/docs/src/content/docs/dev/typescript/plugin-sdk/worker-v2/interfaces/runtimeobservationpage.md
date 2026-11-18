---
title: "plugin-sdk: worker-v2/interfaces/RuntimeObservationPage"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / RuntimeObservationPage

# Interface: RuntimeObservationPage

A bounded page of latest cumulative observations in the granted scope.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="cursor"></a> `cursor` | `string` | Replay cursor. Persist it with the accepted observations. |
| <a id="more"></a> `more` | `boolean` | Read another page while true. |
| <a id="observations"></a> `observations` | [`RuntimeObservation`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimeobservation/)[] | Rows ordered by the host's durable observation sequence. |
| <a id="reset_required"></a> `reset_required` | `boolean` | This cursor is outside retention. Restart from zero and reconcile. |
