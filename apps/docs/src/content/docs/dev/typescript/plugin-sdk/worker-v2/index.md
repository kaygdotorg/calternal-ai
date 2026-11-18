---
title: "plugin-sdk: worker-v2/README"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / worker-v2

# worker-v2

Bounded worker-v2 peer; effects and current authority remain in the trusted host.

## Classes

- [WorkerRuntimeV2](/docs/dev/typescript/plugin-sdk/worker-v2/classes/workerruntimev2/)

## Interfaces

- [InvocationAuthority](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/invocationauthority/)
- [PluginState](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/pluginstate/)
- [ResourceAdmissionInput](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/resourceadmissioninput/)
- [ResourceOwner](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/resourceowner/)
- [RuntimeObservation](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimeobservation/)
- [RuntimeObservationPage](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimeobservationpage/)
- [RuntimeWindow](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimewindow/)
- [SpaceUsage](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/spaceusage/)
- [WorkerContextV2](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workercontextv2/)
- [WorkerFrameV2](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workerframev2/)

## Type Aliases

- [ResourceAdmissionDecision](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/resourceadmissiondecision/)
- [WorkerHandlerV2](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/workerhandlerv2/)
- [WorkerMessageV2](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/workermessagev2/)

## Variables

- [MAX\_V2\_FRAME\_BYTES](/docs/dev/typescript/plugin-sdk/worker-v2/variables/max_v2_frame_bytes/)
- [MAX\_V2\_PENDING](/docs/dev/typescript/plugin-sdk/worker-v2/variables/max_v2_pending/)

## Functions

- [compareAndSetPluginState](/docs/dev/typescript/plugin-sdk/worker-v2/functions/compareandsetpluginstate/)
- [encodeWorkerFrameV2](/docs/dev/typescript/plugin-sdk/worker-v2/functions/encodeworkerframev2/)
- [listItems](/docs/dev/typescript/plugin-sdk/worker-v2/functions/listitems/)
- [parseWorkerFrameV2](/docs/dev/typescript/plugin-sdk/worker-v2/functions/parseworkerframev2/)
- [prepareContent](/docs/dev/typescript/plugin-sdk/worker-v2/functions/preparecontent/)
- [readPluginState](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readpluginstate/)
- [readRuntimeObservations](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readruntimeobservations/)
- [readRuntimeWindow](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readruntimewindow/)
- [readSetting](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readsetting/)
- [readSpaceUsage](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readspaceusage/)
