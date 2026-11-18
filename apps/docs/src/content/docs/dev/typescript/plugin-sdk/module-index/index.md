---
title: "plugin-sdk: index/README"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / index

# index

## Classes

- [ContractValidationError](/docs/dev/typescript/plugin-sdk/module-index/classes/contractvalidationerror/)

## Type Aliases

- [HostCallKind](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/hostcallkind/)
- [ProviderBilling](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providerbilling/)
- [ProviderCommand](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providercommand/)
- [ProviderContext](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providercontext/)
- [ProviderEvent](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providerevent/)
- [ProviderEventKind](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providereventkind/)
- [ProviderFailureClass](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providerfailureclass/)
- [ProviderRequest](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providerrequest/)

## Variables

- [GENUI\_VERSION](/docs/dev/typescript/plugin-sdk/module-index/variables/genui_version/)
- [MAX\_BODY\_BYTES](/docs/dev/typescript/plugin-sdk/module-index/variables/max_body_bytes/)
- [MAX\_DOCUMENT\_BYTES](/docs/dev/typescript/plugin-sdk/module-index/variables/max_document_bytes/)
- [MAX\_GENUI\_SERIALIZED\_BYTES](/docs/dev/typescript/plugin-sdk/module-index/variables/max_genui_serialized_bytes/)
- [MAX\_WORKER\_STDERR\_BYTES](/docs/dev/typescript/plugin-sdk/module-index/variables/max_worker_stderr_bytes/)
- [PROTOCOL\_VERSION](/docs/dev/typescript/plugin-sdk/module-index/variables/protocol_version/)
- [WORKER\_TIMEOUT\_MS](/docs/dev/typescript/plugin-sdk/module-index/variables/worker_timeout_ms/)

## Functions

- [parseOperationRequest](/docs/dev/typescript/plugin-sdk/module-index/functions/parseoperationrequest/)
- [validateGenUiView](/docs/dev/typescript/plugin-sdk/module-index/functions/validategenuiview/)
- [validateOperationRequest](/docs/dev/typescript/plugin-sdk/module-index/functions/validateoperationrequest/)

## References

### compareAndSetPluginState

Re-exports [compareAndSetPluginState](/docs/dev/typescript/plugin-sdk/worker-v2/functions/compareandsetpluginstate/)

***

### encodeWorkerFrameV2

Re-exports [encodeWorkerFrameV2](/docs/dev/typescript/plugin-sdk/worker-v2/functions/encodeworkerframev2/)

***

### InvocationAuthority

Re-exports [InvocationAuthority](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/invocationauthority/)

***

### JobPark

Re-exports [JobPark](/docs/dev/typescript/plugin-sdk/job-waits/interfaces/jobpark/)

***

### JobSignal

Re-exports [JobSignal](/docs/dev/typescript/plugin-sdk/job-waits/type-aliases/jobsignal/)

***

### JobWaitSource

Re-exports [JobWaitSource](/docs/dev/typescript/plugin-sdk/job-waits/type-aliases/jobwaitsource/)

***

### listItems

Re-exports [listItems](/docs/dev/typescript/plugin-sdk/worker-v2/functions/listitems/)

***

### MAX\_V2\_FRAME\_BYTES

Re-exports [MAX_V2_FRAME_BYTES](/docs/dev/typescript/plugin-sdk/worker-v2/variables/max_v2_frame_bytes/)

***

### MAX\_V2\_PENDING

Re-exports [MAX_V2_PENDING](/docs/dev/typescript/plugin-sdk/worker-v2/variables/max_v2_pending/)

***

### ParkedJob

Re-exports [ParkedJob](/docs/dev/typescript/plugin-sdk/job-waits/interfaces/parkedjob/)

***

### parkJob

Re-exports [parkJob](/docs/dev/typescript/plugin-sdk/job-waits/functions/parkjob/)

***

### parseWorkerFrameV2

Re-exports [parseWorkerFrameV2](/docs/dev/typescript/plugin-sdk/worker-v2/functions/parseworkerframev2/)

***

### PluginState

Re-exports [PluginState](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/pluginstate/)

***

### readPluginState

Re-exports [readPluginState](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readpluginstate/)

***

### readRuntimeObservations

Re-exports [readRuntimeObservations](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readruntimeobservations/)

***

### readRuntimeWindow

Re-exports [readRuntimeWindow](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readruntimewindow/)

***

### readSetting

Re-exports [readSetting](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readsetting/)

***

### readSpaceUsage

Re-exports [readSpaceUsage](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readspaceusage/)

***

### ResourceAdmissionDecision

Re-exports [ResourceAdmissionDecision](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/resourceadmissiondecision/)

***

### ResourceAdmissionInput

Re-exports [ResourceAdmissionInput](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/resourceadmissioninput/)

***

### ResourceOwner

Re-exports [ResourceOwner](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/resourceowner/)

***

### RuntimeObservation

Re-exports [RuntimeObservation](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimeobservation/)

***

### RuntimeObservationPage

Re-exports [RuntimeObservationPage](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimeobservationpage/)

***

### RuntimeWindow

Re-exports [RuntimeWindow](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimewindow/)

***

### serveWorker

Re-exports [serveWorker](/docs/dev/typescript/plugin-sdk/stdio-worker/functions/serveworker/)

***

### SpaceUsage

Re-exports [SpaceUsage](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/spaceusage/)

***

### typedHandlers

Re-exports [typedHandlers](/docs/dev/typescript/plugin-sdk/typed-handlers/functions/typedhandlers/)

***

### validateManifestV1

Re-exports [validateManifestV1](/docs/dev/typescript/plugin-sdk/manifest-v1/functions/validatemanifestv1/)

***

### WorkerContextV2

Re-exports [WorkerContextV2](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workercontextv2/)

***

### WorkerFrameV2

Re-exports [WorkerFrameV2](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workerframev2/)

***

### WorkerHandlerV2

Re-exports [WorkerHandlerV2](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/workerhandlerv2/)

***

### WorkerMessageV2

Re-exports [WorkerMessageV2](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/workermessagev2/)

***

### WorkerRuntimeV2

Re-exports [WorkerRuntimeV2](/docs/dev/typescript/plugin-sdk/worker-v2/classes/workerruntimev2/)
