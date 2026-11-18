//! CLI commands and structured output for the HTTP people directory.
use crate::output::{CliError, Output, Reply};
use calternal_core::PeopleOperation as Op;
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use std::io::Read;
use std::path::PathBuf;

/// A space grant identifies one recipient.
#[derive(Debug, Args)]
pub struct Recipient {
    /// Account UUID for a direct grant.
    #[arg(long, conflicts_with = "group", required_unless_present = "group")]
    account: Option<String>,
    /// Group UUID for a group grant.
    #[arg(long)]
    group: Option<String>,
}

impl Recipient {
    fn input(self, space: String, role: Option<String>) -> Value {
        let mut input = json!({"space_id":space});
        if let Some(account) = self.account {
            input["account_id"] = json!(account);
        }
        if let Some(group) = self.group {
            input["group_id"] = json!(group);
        }
        if let Some(role) = role {
            input["role"] = json!(role);
        }
        input
    }
}

/// Directory actions available through the signed-in account.
#[derive(Debug, Subcommand)]
pub enum People {
    /// List the direct account and group grants in a space.
    Members {
        /// Space UUID.
        #[arg(long)]
        space: String,
        /// Continue from the previous members page.
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Add a person or group to a space.
    Add {
        /// Space UUID.
        #[arg(long)]
        space: String,
        /// The account or group receiving access.
        #[command(flatten)]
        recipient: Recipient,
        /// Reader, Editor, or Manager (lowercase).
        #[arg(long)]
        role: String,
    },
    /// Change a person's or group's direct space role.
    SetRole {
        /// Space UUID.
        #[arg(long)]
        space: String,
        /// The grant whose role changes.
        #[command(flatten)]
        recipient: Recipient,
        /// Reader, Editor, or Manager (lowercase).
        #[arg(long)]
        role: String,
    },
    /// Remove a person's or group's direct grant.
    Remove {
        /// Space UUID.
        #[arg(long)]
        space: String,
        /// The grant to remove.
        #[command(flatten)]
        recipient: Recipient,
    },
    /// Transfer ownership to a current person in the space.
    Transfer {
        /// Space UUID.
        #[arg(long)]
        space: String,
        /// Account UUID of the new owner.
        account: String,
    },
    /// Permanently erase your account or, as a server admin, another account.
    #[command(
        after_help = "Verify your passkey in a browser or native installation before deleting an account. Transfer shared spaces with calternal people transfer first."
    )]
    Delete {
        /// Omit to delete your own account.
        #[arg(long)]
        account: Option<String>,
    },
    /// Show the available space roles and their capabilities.
    Roles,
    /// Change your own account display name.
    SetProfile {
        /// Display name.
        name: String,
    },
    /// List server accounts. Requires server people administration.
    List {
        /// Continue from the previous people page.
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Disable a server account and sign it out.
    Disable {
        /// Account UUID.
        account: String,
    },
}

/// Server-managed local groups.
#[derive(Debug, Subcommand)]
pub enum Groups {
    /// List groups and their active member IDs.
    List {
        /// Continue the members of this group, instead of listing groups.
        #[arg(long)]
        group: Option<String>,
        /// Continue from the last group page or the selected group's member page.
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Create a local group.
    Create {
        /// Display name.
        name: String,
    },
    /// Rename a local group.
    Rename {
        /// Group UUID.
        group: String,
        /// New display name.
        name: String,
    },
    /// Delete a local group and revoke its group grants.
    Delete {
        /// Group UUID.
        group: String,
    },
    /// Add a person to a local group.
    AddMember {
        /// Group UUID.
        group: String,
        /// Account UUID.
        account: String,
    },
    /// Remove a person from a local group.
    RemoveMember {
        /// Group UUID.
        group: String,
        /// Account UUID.
        account: String,
    },
}

/// Single-use invitations for joining a space.
#[derive(Debug, Subcommand)]
pub enum Invites {
    /// Create an expiring link. List invites to copy the link.
    Create {
        /// Space UUID.
        #[arg(long)]
        space: String,
        /// Reader, Editor, or Manager (lowercase).
        #[arg(long)]
        role: String,
        /// Absolute Unix milliseconds, omitted for seven days.
        #[arg(long)]
        expires_at: Option<i64>,
    },
    /// List invitations and show usable links.
    List {
        /// Space UUID.
        #[arg(long)]
        space: String,
        /// Continue from the previous invitation page.
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Revoke an unused invitation.
    Revoke {
        /// Space UUID.
        #[arg(long)]
        space: String,
        /// Invitation record UUID.
        invite: String,
    },
    /// Redeem an invite as the signed-in account. New people register a passkey in the browser.
    Redeem {
        /// Read the invite link or secret from a private file or - for stdin.
        #[arg(long)]
        secret_file: PathBuf,
    },
}

impl People {
    /// Convert to the shared operation and input without I/O.
    pub fn operation(self) -> (&'static str, Value) {
        match self {
            Self::Members { space, cursor } => (
                Op::ListSpaceMembers.id(),
                json!({"space_id":space,"cursor":cursor}),
            ),
            Self::Add {
                space,
                recipient,
                role,
            } => (Op::AddSpaceMember.id(), recipient.input(space, Some(role))),
            Self::SetRole {
                space,
                recipient,
                role,
            } => (Op::SetSpaceRole.id(), recipient.input(space, Some(role))),
            Self::Remove { space, recipient } => {
                (Op::RemoveSpaceMember.id(), recipient.input(space, None))
            }
            Self::Transfer { space, account } => (
                Op::TransferOwnership.id(),
                json!({"space_id":space,"account_id":account}),
            ),
            Self::Delete { account } => (
                calternal_core::ACCOUNT_ERASURE_ACTION,
                json!({"account_id":account}),
            ),
            Self::Roles => (Op::ListRoles.id(), json!({})),
            Self::SetProfile { name } => (Op::SetProfile.id(), json!({"name":name})),
            Self::List { cursor } => (Op::ListPeople.id(), json!({"cursor":cursor})),
            Self::Disable { account } => (Op::DisablePerson.id(), json!({"account_id":account})),
        }
    }
}

impl Groups {
    /// Convert to the shared operation and input without I/O.
    pub fn operation(self) -> (&'static str, Value) {
        let (op, input) = match self {
            Self::List { group, cursor } => {
                (Op::ListGroups, json!({"group_id":group,"cursor":cursor}))
            }
            Self::Create { name } => (Op::CreateGroup, json!({"name":name})),
            Self::Rename { group, name } => {
                (Op::RenameGroup, json!({"group_id":group,"name":name}))
            }
            Self::Delete { group } => (Op::DeleteGroup, json!({"group_id":group})),
            Self::AddMember { group, account } => (
                Op::AddGroupMember,
                json!({"group_id":group,"account_id":account}),
            ),
            Self::RemoveMember { group, account } => (
                Op::RemoveGroupMember,
                json!({"group_id":group,"account_id":account}),
            ),
        };
        (op.id(), input)
    }
}

impl Invites {
    /// Convert to the shared operation. Redemption reads at most 4097 bytes and
    /// accepts a link or raw secret. The credential never needs an argv value.
    pub fn operation(self) -> Result<(&'static str, Value), CliError> {
        let (op, input) = match self {
            Self::Create {
                space,
                role,
                expires_at,
            } => (
                Op::CreateInvite,
                json!({"space_id":space,"role":role,"expires_at":expires_at}),
            ),
            Self::List { space, cursor } => {
                (Op::ListInvites, json!({"space_id":space,"cursor":cursor}))
            }
            Self::Revoke { space, invite } => (
                Op::RevokeInvite,
                json!({"space_id":space,"invite_id":invite}),
            ),
            Self::Redeem { secret_file } => {
                let read: Box<dyn Read> = if secret_file.as_os_str() == "-" {
                    Box::new(std::io::stdin())
                } else {
                    Box::new(
                        std::fs::File::open(secret_file)
                            .map_err(|_| CliError::usage("Cannot open invite file."))?,
                    )
                };
                let mut text = String::new();
                read.take(4097)
                    .read_to_string(&mut text)
                    .map_err(|_| CliError::usage("Cannot read the invite as UTF-8."))?;
                if text.len() > 4096 {
                    return Err(CliError::usage("Invite file is too large."));
                }
                let trimmed = text.trim();
                let secret = trimmed
                    .rsplit_once("/invites/")
                    .map_or(trimmed, |(_, secret)| secret);
                (Op::RedeemInvite, json!({"secret":secret}))
            }
        };
        Ok((op.id(), input))
    }
}

/// Render known directory operations with shared adaptive tables and stable JSON.
/// Returns `None` for other operations. Cost scales with the displayed rows.
pub fn render(out: &Output, operation: &str, id: &str, data: &Value) -> Option<Reply> {
    let reply = Reply::new(data.clone()).request_id(id);
    let text = |v: &Value, key: &str| cell(&v[key]);
    let values = |key: &str| data[key].as_array().into_iter().flatten();
    let reply = if operation == "spaces.members" {
        let people = values("members").map(|r| {
            vec![
                text(r, "account_id"),
                text(r, "name"),
                text(r, "role"),
                "Person".into(),
                "—".into(),
            ]
        });
        let groups = values("groups").map(|r| {
            vec![
                text(r, "group_id"),
                text(r, "name"),
                text(r, "role"),
                "Group".into(),
                text(r, "member_count"),
            ]
        });
        reply.table(
            &["ID", "NAME", "ROLE", "TYPE", "MEMBERS"],
            people.chain(groups).collect(),
            "No member grants.",
        )
    } else if operation == "spaces.transfer" {
        reply.line("Ownership transferred.")
    } else {
        match Op::from_id(operation)? {
            Op::ListSpaceMembers | Op::TransferOwnership => unreachable!(),
            Op::ListRoles => {
                let mut reply = reply;
                for role in values("roles") {
                    reply = reply.fields([("role", text(role, "name"))]).table(
                        &["CAPABILITY"],
                        role["capabilities"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|cap| vec![cell(cap)])
                            .collect(),
                        "No capabilities.",
                    );
                }
                reply
            }
            Op::ListPeople => reply.table(
                &["ID", "NAME", "ROLE", "STATE"],
                values("people")
                    .map(|r| {
                        vec![
                            text(r, "account_id"),
                            text(r, "name"),
                            text(r, "role"),
                            if r["disabled"] == true {
                                "Disabled"
                            } else {
                                "Active"
                            }
                            .into(),
                        ]
                    })
                    .collect(),
                "No server accounts.",
            ),
            Op::ListGroups => {
                let mut reply = reply;
                for group in values("groups") {
                    reply = reply
                        .table(
                            &["ID", "NAME", "SOURCE"],
                            vec![vec![
                                text(group, "group_id"),
                                text(group, "name"),
                                text(group, "source"),
                            ]],
                            "",
                        )
                        .table(
                            &["MEMBER ID"],
                            group["members"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .map(|member| vec![cell(member)])
                                .collect(),
                            "No active members.",
                        );
                }
                for group in values("groups") {
                    if let Some(cursor) = group["members_next_cursor"].as_str() {
                        reply = reply
                            .fields([
                                ("group", text(group, "group_id")),
                                ("member cursor", cursor.to_owned()),
                            ])
                            .hint("Next: use groups list --group GROUP --cursor CURSOR.");
                    }
                }
                if data["groups"].as_array().is_some_and(Vec::is_empty) {
                    reply.line("No server groups.")
                } else {
                    reply
                }
            }
            Op::ListInvites => {
                let format_date = |r: &Value, key: &str| {
                    r[key]
                        .as_i64()
                        .map(|ms| out.local_ms(ms))
                        .unwrap_or_else(|| "—".into())
                };
                let mut reply = reply.table(
                    &["ID", "ROLE", "CREATED", "EXPIRES", "USED", "REVOKED"],
                    values("invites")
                        .map(|r| {
                            vec![
                                text(r, "invite_id"),
                                text(r, "role"),
                                format_date(r, "created_at"),
                                format_date(r, "expires_at"),
                                format_date(r, "used_at"),
                                format_date(r, "revoked_at"),
                            ]
                        })
                        .collect(),
                    "No current invitations.",
                );
                for r in values("invites").filter(|r| r["link"].is_string()) {
                    reply = reply
                        .blank()
                        .fields([("invite", text(r, "invite_id"))])
                        .text(text(r, "link"));
                }
                reply
            }
            Op::CreateInvite => reply
                .fields([
                    ("invite", text(data, "invite_id")),
                    (
                        "expires",
                        data["expires_at"]
                            .as_i64()
                            .map(|ms| out.local_ms(ms))
                            .unwrap_or_default(),
                    ),
                ])
                .hint("Next: calternal invites list --space SPACE_UUID"),
            Op::RevokeInvite => reply
                .line("Invitation revoked.")
                .fields([("invite", text(data, "invite_id"))]),
            Op::RedeemInvite => reply
                .line("Joined space.")
                .fields([("space", text(data, "space_id"))]),
            Op::SetProfile => reply.fields([
                ("account", text(data, "account_id")),
                ("name", text(data, "name")),
            ]),
            Op::DisablePerson => reply
                .line("Account disabled.")
                .fields([("account", text(data, "account_id"))]),
            Op::AddSpaceMember => reply.line("Access granted."),
            Op::SetSpaceRole => reply.line("Role saved."),
            Op::RemoveSpaceMember => reply.line("Direct access removed."),
            Op::CreateGroup
            | Op::RenameGroup
            | Op::DeleteGroup
            | Op::AddGroupMember
            | Op::RemoveGroupMember => reply
                .fields([("group", text(data, "group_id"))])
                .hint("Next: calternal groups list"),
        }
    };
    Some(reply.page_cursor())
}

fn cell(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
