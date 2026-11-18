//! People operation dispatch, with scoped receipts and sanitized authority errors.
use super::Shared;
use super::dispatch::{fingerprint, space_of};
use crate::auth::{Directory, Identity};
use calternal_auth::people::GroupChange;
use calternal_auth::spaces::{MemberTarget, SpaceChange};
use calternal_core::{JsonObject, OperationRisk, PeopleOperation, PeopleTarget, SpaceGrantInput};
use calternal_types::{AccountId, Error, GroupId, InviteId};
use serde_json::Value;

pub(super) async fn dispatch<D: Directory>(
    state: Shared<D>,
    person: Identity,
    operation: PeopleOperation,
    request_id: String,
    input: JsonObject,
) -> Result<Value, Error> {
    tokio::task::spawn_blocking(move || {
        let op = operation.id();
        calternal_core::validate_operation_input(op, &input)?;
        let auth = &state
            .directory
            .http_auth()
            .ok_or_else(|| Error::unavailable("people service unavailable"))?
            .auth;
        let spaces = auth.spaces();
        let actor = person.id;
        let fp = fingerprint(op, &input);
        let receipt = (request_id.as_str(), op, fp.as_str());
        let text = |key: &str| field(&input, key);
        let cursor = input.get("cursor").and_then(Value::as_str);
        let account = || AccountId::parse(text("account_id")?);
        let group = || GroupId::parse(text("group_id")?);
        let space = if operation.target() == PeopleTarget::Space {
            Some(space_of(text("space_id")?)?)
        } else {
            None
        };
        let execute = || {
            let scope = || space.ok_or_else(|| Error::not_found("space not found"));
            let result = match operation {
                PeopleOperation::ListRoles => spaces.roles_list(actor),
                PeopleOperation::SetProfile => {
                    spaces.set_profile(actor, text("name")?.to_owned(), receipt)
                }
                PeopleOperation::ListPeople => {
                    spaces.people_list(actor, cursor.map(AccountId::parse).transpose()?)
                }
                PeopleOperation::DisablePerson => auth.disable_person(actor, account()?, receipt),
                PeopleOperation::ListGroups
                    if input.get("group_id").is_some_and(Value::is_string) =>
                {
                    spaces.group_members_page(
                        actor,
                        group()?,
                        cursor.map(AccountId::parse).transpose()?,
                    )
                }
                PeopleOperation::ListGroups => {
                    spaces.groups_list(actor, cursor.map(GroupId::parse).transpose()?)
                }
                PeopleOperation::CreateGroup => spaces.change_group(
                    actor,
                    GroupChange::Create {
                        name: text("name")?.to_owned(),
                    },
                    receipt,
                ),
                PeopleOperation::RenameGroup => spaces.change_group(
                    actor,
                    GroupChange::Rename {
                        group: group()?,
                        name: text("name")?.to_owned(),
                    },
                    receipt,
                ),
                PeopleOperation::DeleteGroup => {
                    spaces.change_group(actor, GroupChange::Delete(group()?), receipt)
                }
                PeopleOperation::AddGroupMember => spaces.change_group(
                    actor,
                    GroupChange::AddMember {
                        group: group()?,
                        account: account()?,
                    },
                    receipt,
                ),
                PeopleOperation::RemoveGroupMember => spaces.change_group(
                    actor,
                    GroupChange::RemoveMember {
                        group: group()?,
                        account: account()?,
                    },
                    receipt,
                ),
                PeopleOperation::CreateInvite => auth.create_invite(
                    actor,
                    scope()?,
                    text("role")?.to_owned(),
                    input.get("expires_at").and_then(Value::as_i64),
                    receipt,
                ),
                PeopleOperation::ListInvites => {
                    auth.list_invites(actor, scope()?, cursor.map(InviteId::parse).transpose()?)
                }
                PeopleOperation::RevokeInvite => auth.revoke_invite(
                    actor,
                    scope()?,
                    InviteId::parse(text("invite_id")?)?,
                    receipt,
                ),
                PeopleOperation::RedeemInvite => {
                    auth.redeem_invite(actor, text("secret")?.to_owned(), receipt)
                }
                PeopleOperation::ListSpaceMembers
                | PeopleOperation::TransferOwnership
                | PeopleOperation::AddSpaceMember
                | PeopleOperation::SetSpaceRole
                | PeopleOperation::RemoveSpaceMember => {
                    return Err(Error::internal("space grant must use directory dispatch"));
                }
            };
            result.map_err(auth_error)
        };
        if let Some(space) = space
            && operation.risk() != OperationRisk::Read
        {
            state.store.content().admit(
                space,
                calternal_content::Request {
                    account: actor,
                    via: None,
                    id: &request_id,
                    operation: op,
                    fingerprint: &fp,
                },
                execute,
            )
        } else {
            execute()
        }
    })
    .await
    .map_err(|_| Error::internal("people directory task failed"))?
}

pub(crate) fn grant_change(
    operation: PeopleOperation,
    input: &JsonObject,
) -> Result<SpaceChange, Error> {
    let account = input.get("account_id").and_then(Value::as_str);
    let group = input.get("group_id").and_then(Value::as_str);
    SpaceGrantInput::validate_selection(account.is_some(), group.is_some())?;
    let target = if let Some(account) = account {
        MemberTarget::Account(AccountId::parse(account)?)
    } else {
        MemberTarget::Group(GroupId::parse(field(input, "group_id")?)?)
    };
    let role = || field(input, "role").map(str::to_owned);
    match (operation, target) {
        (PeopleOperation::AddSpaceMember, target) => Ok(SpaceChange::Add {
            target,
            role: role()?,
        }),
        (PeopleOperation::SetSpaceRole, MemberTarget::Account(target)) => {
            Ok(SpaceChange::SetRole {
                target,
                role: role()?,
            })
        }
        (PeopleOperation::SetSpaceRole, MemberTarget::Group(target)) => {
            Ok(SpaceChange::SetGroupRole {
                target,
                role: role()?,
            })
        }
        (PeopleOperation::RemoveSpaceMember, MemberTarget::Account(target)) => {
            Ok(SpaceChange::Remove { target })
        }
        (PeopleOperation::RemoveSpaceMember, MemberTarget::Group(target)) => {
            Ok(SpaceChange::RemoveGroup { target })
        }
        _ => Err(Error::not_found("unknown operation")),
    }
}

fn field<'a>(input: &'a JsonObject, name: &str) -> Result<&'a str, Error> {
    input
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::invalid_input(format!("provide {name}")))
}

pub(crate) fn auth_error(error: calternal_auth::Error) -> Error {
    use calternal_auth::ErrorCode;
    match error.code() {
        ErrorCode::NotFound => Error::not_found("directory entry not found"),
        ErrorCode::SetupTokenInvalid => Error::not_found("invite invalid or expired"),
        ErrorCode::Forbidden => Error::forbidden("insufficient authority"),
        ErrorCode::InvalidInput => {
            Error::invalid_input("check the name, role or expiry and try again")
        }
        ErrorCode::InviteLimit => Error::conflict("this space has reached its live invitation limit")
            .with_details(serde_json::json!({"reason":"invite_limit","limit":calternal_auth::invites::MAX_LIVE_SPACE_INVITES})),
        ErrorCode::Conflict => Error::request_reused(),
        _ => Error::internal("people database operation failed"),
    }
}

pub(super) async fn development_identity(
    directory: &impl Directory,
    headers: &axum::http::HeaderMap,
) -> Option<Identity> {
    let identity = directory.authenticate(super::guard::bearer(headers)?)?;
    if let Some(service) = directory.http_auth()
        && !service.auth.development_account_active(identity.id).await
    {
        return None;
    }
    Some(identity)
}
