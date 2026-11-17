//! Typed associations reuse core wire DTOs without introducing another schema.
use super::*;
/// An operation name paired with its generated wire input and result types.
/// Markers allocate nothing and perform no I/O.
/// Dynamic plugins can still use [`Client::execute_cancellable`] with runtime schemas.
pub trait Operation {
    /// Generated contract input serialized as an object; encoding is O(input bytes).
    type Input: Serialize;
    /// Generated contract result decoded after correlation checks; O(result bytes).
    type Output: DeserializeOwned;
    /// Exact registry operation name; static storage, no allocation.
    const NAME: &'static str;
}

/// Immutable typed request retained across retries and uncertain outcomes.
/// Holds O(input bytes) in memory; no locks or I/O. This is not a durable offline outbox.
pub struct PendingCall<O: Operation> {
    request: PendingRequest,
    marker: std::marker::PhantomData<fn() -> O>,
}
impl<O: Operation> PendingCall<O> {
    /// Validate input and allocate one fresh identity; O(input bytes) allocation and
    /// validation plus UUID entropy, no network/persistent writes. Invalid input fails locally.
    pub fn new(input: &O::Input) -> Result<Self, ClientError> {
        Self::with_id(input, None)
    }
    /// Reconstruct a retained request with an explicit identity; callers must retain
    /// identical operation/input alongside this identity, scoped to server and account.
    /// O(input bytes) validation/allocation, no network/persistent I/O; fresh IDs
    /// use UUID entropy. Returns local contract errors.
    pub fn with_id(input: &O::Input, request_id: Option<String>) -> Result<Self, ClientError> {
        Ok(Self {
            request: Client::pending(O::NAME, input, request_id)?,
            marker: std::marker::PhantomData,
        })
    }
    /// Borrow the envelope in O(1), without allocation, lock or I/O.
    pub fn request(&self) -> &OperationRequest {
        self.request.request()
    }
}

/// Typed operation markers associated with the core's generated wire contracts.
pub mod operations {
    use super::*;
    macro_rules! operation {
        ($name:ident, $wire:literal, $input:ty, $output:ty) => {
            #[doc = concat!("Contract association for `", $wire, "`; zero-sized marker with no allocation or I/O; the server checks authority.")]
            pub enum $name {}
            impl Operation for $name {
                type Input = $input;
                type Output = $output;
                const NAME: &'static str = $wire;
            }
        };
    }
    operation!(Resolve, "resolve", ResolveInput, ResolveResult);
    operation!(SpacesList, "spaces.list", SpacesListInput, SpacesListResult);
    operation!(
        SpacesCreate,
        "spaces.create",
        SpacesCreateInput,
        SpaceResult
    );
    operation!(
        SpacesMembers,
        "spaces.members",
        SpaceInput,
        SpaceMembersResult
    );
    operation!(
        SpacesSetRole,
        "spaces.set_role",
        SpaceGrantRoleInput,
        SpaceChangedResult
    );
    operation!(
        SpacesRemoveMember,
        "spaces.remove_member",
        SpaceGrantInput,
        SpaceChangedResult
    );
    operation!(SpacesLeave, "spaces.leave", SpaceInput, SpaceChangedResult);
    operation!(
        SpacesTransfer,
        "spaces.transfer",
        SpaceMemberInput,
        SpaceChangedResult
    );
    operation!(
        SpacesDelete,
        "spaces.delete",
        SpaceInput,
        SpaceChangedResult
    );
    operation!(NotesList, "notes.list", NotesListInput, NotesListResult);
    operation!(NotesGet, "notes.get", NotesGetInput, NoteResult);
    operation!(NotesCreate, "notes.create", NotesCreateInput, NoteResult);
    operation!(NotesUpdate, "notes.update", NotesUpdateInput, NoteResult);
    operation!(
        NotesHistory,
        "notes.history",
        NotesHistoryInput,
        NotesHistoryResult
    );
}
