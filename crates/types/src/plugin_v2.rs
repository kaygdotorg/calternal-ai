//! Shared identities for the v2 registry, item broker, and execution engine.

use serde::{Deserialize, Serialize};

macro_rules! identity {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);
    };
}

identity!(
    /// Logical manifest package identifier. A package ID alone proves neither
    /// the publisher nor the installed build.
    PackageId
);
identity!(
    /// Contribution identifier local to its declaring package.
    ContributionId
);

/// Stable logical identity of a contribution across installs. An execution
/// requires a separately checked binding to an exact installed build.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContributionIdentity {
    /// Package that declares the contribution.
    pub package: PackageId,
    /// Contribution within that package.
    pub contribution: ContributionId,
}

identity!(
    /// Host-issued item reference. Use requires the current operation's
    /// space, kind and item authority checks.
    ItemRef
);
identity!(
    /// Opaque reference to one item revision. Resolve it in the context of
    /// its item before using it as a read or write fence.
    RevisionRef
);
identity!(
    /// Durable identifier of an admitted job record, across claim and retry.
    JobId
);
identity!(
    /// Opaque reference to an exact admitted execution job and its pinned
    /// target item and audience generation.
    ExecutionIntentRef
);
identity!(
    /// Opaque reference to one execution attempt. A new attempt requires a
    /// fresh reference even when the admitted job is unchanged.
    ExecutionRef
);
identity!(
    /// Opaque lease identity. Its holder, epoch, expiry and scope are
    /// checked by the engine that issued it.
    LeaseRef
);
identity!(
    /// Request key supplied for replay of a fenced operation. The receiver
    /// uses the operation and actor scope when looking up a receipt.
    RequestId
);
identity!(
    /// Host-issued receipt reference for a committed effect within the
    /// receipt's operation and retention scope.
    ReceiptRef
);
identity!(
    /// Host-issued binding to the installed harness contribution, build,
    /// extension point, account and space. Live authority is checked at use.
    HarnessRef
);
identity!(
    /// Opaque host-issued execution placement choice. It is a ChoiceRef on
    /// the wire, bound to its checked actor, harness and choice scope.
    PlacementRef
);
