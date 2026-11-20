---
title: "plugin-sdk: index/README"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

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

### AnyOfSchema

Re-exports [AnyOfSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/anyofschema/)

***

### appendMessage

Re-exports [appendMessage](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/appendmessage/)

***

### array

Re-exports [array](/docs/dev/typescript/plugin-sdk/schema/functions/array/)

***

### ArrayKeywords

Re-exports [ArrayKeywords](/docs/dev/typescript/plugin-sdk/schema/interfaces/arraykeywords/)

***

### ArraySchema

Re-exports [ArraySchema](/docs/dev/typescript/plugin-sdk/schema/type-aliases/arrayschema/)

***

### authorContext

Re-exports [authorContext](/docs/dev/typescript/plugin-sdk/operation/functions/authorcontext/)

***

### BodySave

Re-exports [BodySave](/docs/dev/typescript/plugin-sdk/broker-helpers/type-aliases/bodysave/)

***

### Broker

Re-exports [Broker](/docs/dev/typescript/plugin-sdk/operation/type-aliases/broker/)

***

### BrokerInput

Re-exports [BrokerInput](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokerinput/)

***

### BrokerMethod

Re-exports [BrokerMethod](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokermethod/)

***

### BrokerOutput

Re-exports [BrokerOutput](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokeroutput/)

***

### BrokerReplyError

Re-exports [BrokerReplyError](/docs/dev/typescript/plugin-sdk/broker-helpers/classes/brokerreplyerror/)

***

### CatalogueName

Re-exports [CatalogueName](/docs/dev/typescript/plugin-sdk/schema/type-aliases/cataloguename/)

***

### CatalogueRefSchema

Re-exports [CatalogueRefSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/cataloguerefschema/)

***

### checkItemView

Re-exports [checkItemView](/docs/dev/typescript/plugin-sdk/limits/functions/checkitemview/)

***

### checkLimit

Re-exports [checkLimit](/docs/dev/typescript/plugin-sdk/limits/functions/checklimit/)

***

### ConstSchema

Re-exports [ConstSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/constschema/)

***

### ContributionEndedError

Re-exports [ContributionEndedError](/docs/dev/typescript/plugin-sdk/broker-helpers/classes/contributionendederror/)

***

### ContributionOutput

Re-exports [ContributionOutput](/docs/dev/typescript/plugin-sdk/broker-helpers/interfaces/contributionoutput/)

***

### defineApp

Re-exports [defineApp](/docs/dev/typescript/plugin-sdk/author-v2/functions/defineapp/)

***

### defineChrome

Re-exports [defineChrome](/docs/dev/typescript/plugin-sdk/author-v2/functions/definechrome/)

***

### DefinedOperation

Re-exports [DefinedOperation](/docs/dev/typescript/plugin-sdk/operation/type-aliases/definedoperation/)

***

### defineKind

Re-exports [defineKind](/docs/dev/typescript/plugin-sdk/author-v2/functions/definekind/)

***

### defineOperation

Re-exports [defineOperation](/docs/dev/typescript/plugin-sdk/operation/functions/defineoperation/)

***

### definePlugin

Re-exports [definePlugin](/docs/dev/typescript/plugin-sdk/author-v2/functions/defineplugin/)

***

### defineView

Re-exports [defineView](/docs/dev/typescript/plugin-sdk/author-v2/functions/defineview/)

***

### encodeContributionFrame

Re-exports [encodeContributionFrame](/docs/dev/typescript/plugin-sdk/limits/functions/encodecontributionframe/)

***

### encodeWorkerFrameV2

Re-exports [encodeWorkerFrameV2](/docs/dev/typescript/plugin-sdk/worker-v2/functions/encodeworkerframev2/)

***

### EnumSchema

Re-exports [EnumSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/enumschema/)

***

### formField

Re-exports [formField](/docs/dev/typescript/plugin-sdk/operation/functions/formfield/)

***

### handlersOf

Re-exports [handlersOf](/docs/dev/typescript/plugin-sdk/operation/functions/handlersof/)

***

### HostMethod

Re-exports [HostMethod](/docs/dev/typescript/plugin-sdk/operation/type-aliases/hostmethod/)

***

### InputOf

Re-exports [InputOf](/docs/dev/typescript/plugin-sdk/operation/type-aliases/inputof/)

***

### integer

Re-exports [integer](/docs/dev/typescript/plugin-sdk/schema/functions/integer/)

***

### IntegerKeywords

Re-exports [IntegerKeywords](/docs/dev/typescript/plugin-sdk/schema/interfaces/integerkeywords/)

***

### InvocationAuthority

Re-exports [InvocationAuthority](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/invocationauthority/)

***

### ItemsKeyword

Re-exports [ItemsKeyword](/docs/dev/typescript/plugin-sdk/schema/interfaces/itemskeyword/)

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

### LIMIT\_SOURCES

Re-exports [LIMIT_SOURCES](/docs/dev/typescript/plugin-sdk/generated/limits/variables/limit_sources/)

***

### LimitError

Re-exports [LimitError](/docs/dev/typescript/plugin-sdk/limits/classes/limiterror/)

***

### LimitName

Re-exports [LimitName](/docs/dev/typescript/plugin-sdk/generated/limits/type-aliases/limitname/)

***

### Limits

Re-exports [Limits](/docs/dev/typescript/plugin-sdk/generated/limits/interfaces/limits/)

***

### LIMITS

Re-exports [LIMITS](/docs/dev/typescript/plugin-sdk/generated/limits/variables/limits/)

***

### LimitSource

Re-exports [LimitSource](/docs/dev/typescript/plugin-sdk/generated/limits/interfaces/limitsource/)

***

### listItems

Re-exports [listItems](/docs/dev/typescript/plugin-sdk/worker-v2/functions/listitems/)

***

### literal

Re-exports [literal](/docs/dev/typescript/plugin-sdk/schema/functions/literal/)

***

### ManifestOperation

Re-exports [ManifestOperation](/docs/dev/typescript/plugin-sdk/operation/type-aliases/manifestoperation/)

***

### MAX\_V2\_FRAME\_BYTES

Re-exports [MAX_V2_FRAME_BYTES](/docs/dev/typescript/plugin-sdk/worker-v2/variables/max_v2_frame_bytes/)

***

### MAX\_V2\_PENDING

Re-exports [MAX_V2_PENDING](/docs/dev/typescript/plugin-sdk/worker-v2/variables/max_v2_pending/)

***

### MessageAppend

Re-exports [MessageAppend](/docs/dev/typescript/plugin-sdk/broker-helpers/type-aliases/messageappend/)

***

### MessageReceipt

Re-exports [MessageReceipt](/docs/dev/typescript/plugin-sdk/broker-helpers/interfaces/messagereceipt/)

***

### object

Re-exports [object](/docs/dev/typescript/plugin-sdk/schema/functions/object/)

***

### ObjectSchema

Re-exports [ObjectSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/objectschema/)

***

### oneOf

Re-exports [oneOf](/docs/dev/typescript/plugin-sdk/schema/functions/oneof/)

***

### OneOfSchema

Re-exports [OneOfSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/oneofschema/)

***

### oneOfValues

Re-exports [oneOfValues](/docs/dev/typescript/plugin-sdk/schema/functions/oneofvalues/)

***

### OperationContext

Re-exports [OperationContext](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)

***

### OperationHandler

Re-exports [OperationHandler](/docs/dev/typescript/plugin-sdk/operation/type-aliases/operationhandler/)

***

### OutputOf

Re-exports [OutputOf](/docs/dev/typescript/plugin-sdk/operation/type-aliases/outputof/)

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

### PropertiesKeyword

Re-exports [PropertiesKeyword](/docs/dev/typescript/plugin-sdk/schema/interfaces/propertieskeyword/)

***

### publishView

Re-exports [publishView](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/publishview/)

***

### readBody

Re-exports [readBody](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/readbody/)

***

### readSetting

Re-exports [readSetting](/docs/dev/typescript/plugin-sdk/worker-v2/functions/readsetting/)

***

### readSpans

Re-exports [readSpans](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/readspans/)

***

### ref

Re-exports [ref](/docs/dev/typescript/plugin-sdk/schema/functions/ref/)

***

### RequiredKeyword

Re-exports [RequiredKeyword](/docs/dev/typescript/plugin-sdk/schema/interfaces/requiredkeyword/)

***

### saveBody

Re-exports [saveBody](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/savebody/)

***

### Schema

Re-exports [Schema](/docs/dev/typescript/plugin-sdk/schema/type-aliases/schema/)

***

### SchemaType

Re-exports [SchemaType](/docs/dev/typescript/plugin-sdk/schema/type-aliases/schematype/)

***

### serveWorker

Re-exports [serveWorker](/docs/dev/typescript/plugin-sdk/stdio-worker/functions/serveworker/)

***

### SpanIndex

Re-exports [SpanIndex](/docs/dev/typescript/plugin-sdk/broker-helpers/type-aliases/spanindex/)

***

### startJob

Re-exports [startJob](/docs/dev/typescript/plugin-sdk/job-waits/functions/startjob/)

***

### streamContribution

Re-exports [streamContribution](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/streamcontribution/)

***

### string

Re-exports [string](/docs/dev/typescript/plugin-sdk/schema/functions/string/)

***

### StringKeywords

Re-exports [StringKeywords](/docs/dev/typescript/plugin-sdk/schema/interfaces/stringkeywords/)

***

### subeffectKey

Re-exports [subeffectKey](/docs/dev/typescript/plugin-sdk/operation/functions/subeffectkey/)

***

### typedHandlers

Re-exports [typedHandlers](/docs/dev/typescript/plugin-sdk/typed-handlers/functions/typedhandlers/)

***

### TypedSchema

Re-exports [TypedSchema](/docs/dev/typescript/plugin-sdk/schema/type-aliases/typedschema/)

***

### TypeKeyword

Re-exports [TypeKeyword](/docs/dev/typescript/plugin-sdk/schema/interfaces/typekeyword/)

***

### utf8Length

Re-exports [utf8Length](/docs/dev/typescript/plugin-sdk/limits/functions/utf8length/)

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
