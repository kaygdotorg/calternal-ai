//! Independent operation inventory and capability declarations, separate from role/race cases.
use super::*;

// Independent account expectations; both rows are exercised by the real router.
const ONBOARDING_MATRIX: &[(&str, bool)] =
    &[("onboarding.state", false), ("onboarding.finish", true)];

#[tokio::test]
async fn onboarding_requires_a_session_and_uses_its_own_account() {
    let fixture = Fixture::new(0, false).await;
    let owner = fixture.state.directory.identity.id.to_string();
    let other = fixture.state.directory.other_identity.id.to_string();

    for &(op, finished) in ONBOARDING_MATRIX {
        let (status, body) = fixture.call(None, "signed-out", op, json!({})).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{op}: {body}");
        let (status, body) = fixture
            .call(
                Some(OTHER_TOKEN),
                "target-owner",
                op,
                json!({"account_id": owner}),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{op}: {body}");
        let (status, body) = fixture
            .call(
                Some(TOKEN),
                "target-other",
                op,
                json!({"account_id": other}),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{op}: {body}");
        let (status, body) = fixture.call(Some(TOKEN), "own", op, json!({})).await;
        assert_eq!(status, StatusCode::OK, "{op}: {body}");
        assert_eq!(body["data"], json!({"finished":finished}), "{op}");
        let (status, body) = fixture
            .call(
                Some(OTHER_TOKEN),
                "other-own",
                "onboarding.state",
                json!({}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"], json!({"finished":false}), "{op}");
    }
}

#[tokio::test]
async fn every_registered_operation_has_an_independent_matrix_row() {
    let fixture = Fixture::new(3, false).await;
    let rows: BTreeSet<_> = MATRIX.iter().map(|(op, _, _)| *op).collect();
    let mut registered: BTreeSet<_> = operation_catalog().into_iter().map(|op| op.id).collect();
    for plugin in fixture.state.registry.snapshot().plugins() {
        registered.extend(plugin.manifest.operations.iter().map(|op| op.id.clone()));
    }
    assert_eq!(rows.len(), MATRIX.len(), "duplicate matrix row");
    // The real stored-directory matrix lives in spaces_matrix.rs, where every
    // directory handler and its target invariants are exercised against SQLite.
    let mut stored_rows: BTreeSet<_> = [
        // Private account authority and target isolation: account_input.rs.
        "drafts.save",
        "drafts.list",
        "drafts.get",
        "drafts.delete",
        "pending.queue",
        "pending.edit",
        "pending.list",
        "pending.cancel",
        "pending.confirm",
        "pending.submit",
        // Exercised with passkeys in tests/conformance/notes_cases.rs.
        "notes.delete",
        "notes.restore",
        "notes.rename",
        "notes.rename.status",
        "notes.rename.undo",
        "notes.lookup",
        "tags.list",
        "tags.lookup",
        "trash.list",
        "trash.purge",
        "trash.empty.preview",
        "trash.empty",
        "trash.empty.status",
        "resolve", // dedicated address matrix in resolver.rs
        // Dedicated item sharing fixtures in resolver.rs.
        "collections.get",
        "collections.list",
        "collections.create",
        "collections.set_members",
        "sharing.list",
        "sharing.mark_seen",
        "sharing.rescue",
        "sharing.audiences",
        "sharing.shared_with_me",
        "sharing.grant",
        "sharing.revoke",
        "sharing.link_create",
        "sharing.link_update",
        "sharing.link_copy",
        "sharing.set_link_policy",
        "spaces.list",
        "wallpapers.list", // signed-in installation catalog in conformance/wallpaper_cases.rs
        "spaces.create",
        "spaces.members",
        // People directory and group grants are exercised by conformance/people_cases.rs.
        "spaces.add_member",
        "spaces.set_role",
        "spaces.remove_member",
        "roles.list",
        "account.set_profile",
        "account.set_time_zone",
        "people.list",
        "people.disable",
        "groups.list",
        "groups.create",
        "groups.rename",
        "groups.delete",
        "groups.add_member",
        "groups.remove_member",
        "invites.create",
        "invites.list",
        "invites.revoke",
        "invites.redeem",
        "spaces.leave",
        "spaces.transfer",
        "spaces.delete",
        // Job rows need a durable job; their role matrix is in plugin_host.rs.
        "jobs.get",
        "jobs.cancel",
        // Account-or-space owned; dedicated stored-directory matrix in secrets.rs.
        "secrets.request",
        "secrets.list",
        "secrets.revoke",
    ]
    .into_iter()
    .collect();
    for &(op, mutates) in ONBOARDING_MATRIX {
        assert!(
            stored_rows.insert(op),
            "duplicate onboarding operation {op}"
        );
        assert_eq!(
            calternal_core::cached_operation(op).unwrap().mutates,
            mutates,
            "{op}"
        );
    }
    // Owner-only installation policy is exercised in conformance/harness_admin_cases.rs.
    for op in [
        "harnesses.list",
        "harnesses.stage",
        "harnesses.activate",
        "harnesses.rollback",
    ] {
        assert!(stored_rows.insert(op), "duplicate harness operation {op}");
        assert_eq!(
            fixture
                .state
                .registry
                .snapshot()
                .policy(op)
                .unwrap()
                .required,
            Capability::ServerSettings,
            "{op}"
        );
    }
    // Manual-turn controls are covered by provider_turns.rs and agent_cases conformance.
    stored_rows.extend(
        calternal_core::AGENT_OPERATIONS
            .iter()
            .map(|(id, _, _)| *id),
    );
    // Request isolation and the instructions role matrix use real passkey
    // sessions in conformance/requests_cases.rs.
    // Package setting access uses the real sessions in plugin_settings_cases.rs.
    for (id, required) in [
        ("plugins.network.content.get", Capability::PluginsUse),
        (
            "plugins.network.content.preference.get",
            Capability::PluginsUse,
        ),
        (
            "plugins.network.content.preference.set",
            Capability::PluginsUse,
        ),
        ("plugins.diagnostics", Capability::PluginsManage),
        ("plugins.personal_settings.get", Capability::SpaceRead),
        ("plugins.personal_settings.set", Capability::SpaceRead),
        ("plugins.settings.get", Capability::SpaceSettings),
        ("plugins.settings.set", Capability::SpaceSettings),
        ("provider.sign_in.start", Capability::PluginsUse),
        ("provider.sign_in.answer", Capability::PluginsUse),
        ("provider.sign_in.status", Capability::PluginsUse),
        ("provider.sign_in.typing", Capability::PluginsUse),
        ("requests.list", Capability::SpaceRead),
        ("requests.ask", Capability::PluginsUse),
        ("requests.answer", Capability::SpaceRead),
        ("requests.approve", Capability::PluginsUse),
        ("requests.deny", Capability::PluginsUse),
        ("space.instructions", Capability::SpaceSettings),
    ] {
        assert!(stored_rows.insert(id), "duplicate request operation {id}");
        assert_eq!(
            fixture
                .state
                .registry
                .snapshot()
                .policy(id)
                .unwrap()
                .required,
            required
        );
    }
    // HTTP behavior is covered with real directory grants in conformance/search_cases.rs.
    use calternal_server::authz::Scope;
    for (id, scope) in [
        ("search.query", Scope::Account),
        ("items.lookup", Scope::Account),
        ("threads.lookup", Scope::Account),
        ("people.lookup", Scope::Space),
        ("skills.list", Scope::Space),
        ("plugins.manifests", Scope::Space),
    ] {
        assert!(stored_rows.insert(id), "duplicate search operation {id}");
        let catalog = fixture.state.registry.snapshot();
        let policy = catalog.policy(id).unwrap();
        assert_eq!(policy.required, Capability::SpaceRead, "{id}");
        assert_eq!(policy.scope, scope, "{id}");
        let declaration = calternal_core::cached_operation(id).unwrap();
        assert_eq!(
            declaration.risk,
            calternal_core::OperationRisk::Read,
            "{id}"
        );
        assert!(!declaration.mutates, "{id}");
    }
    for id in [
        "skills.read",
        "skills.create",
        "skills.update",
        "skills.remove",
        "automations.preview_schedule",
        "automations.list",
        "automations.read",
        "automations.create",
        "automations.update",
        "automations.enable",
        "automations.pause",
        "automations.remove",
        "automations.run_now",
        "automations.next_run",
        "automations.last_run",
    ] {
        assert!(
            stored_rows.insert(id),
            "duplicate automation operation {id}"
        );
        let catalog = fixture.state.registry.snapshot();
        let policy = catalog.policy(id).unwrap();
        assert_eq!(policy.scope, Scope::Space, "{id}");
        let expected = match id {
            "skills.read"
            | "automations.preview_schedule"
            | "automations.list"
            | "automations.read"
            | "automations.next_run"
            | "automations.last_run" => Capability::SpaceRead,
            _ => Capability::ContentWrite,
        };
        assert_eq!(policy.required, expected, "{id}");
    }
    // These handlers use a dedicated person-session dispatcher. Their
    // authorization and account isolation are covered in font_cases.rs.
    let appearance = [
        "appearance.get",
        "appearance.patch",
        "fonts.catalogue",
        "fonts.install",
        "fonts.status",
        "fonts.faces",
        "fonts.list",
        "fonts.remove",
    ];
    for id in appearance {
        assert!(calternal_core::is_appearance_operation(id), "{id}");
        assert!(
            fixture.state.registry.snapshot().policy(id).is_none(),
            "{id}"
        );
        assert!(
            stored_rows.insert(id),
            "duplicate appearance operation {id}"
        );
    }
    let rows: BTreeSet<_> = rows.union(&stored_rows).copied().collect();
    assert_eq!(
        rows.iter()
            .copied()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        registered
    );
    // Host rows plus manifest-derived plugin rows cover every operation exactly once.
    let catalog = fixture.state.registry.snapshot();
    let plugin_rows: Vec<&str> = catalog
        .plugin_policies()
        .map(|p| p.operation.as_ref())
        .collect();
    assert!(
        POLICIES
            .iter()
            .all(|p| !plugin_rows.contains(&p.operation.as_ref())),
        "a plugin row is hardcoded in the host table"
    );
    let all: BTreeSet<String> = POLICIES
        .iter()
        .map(|p| p.operation.to_string())
        .chain(calternal_core::account_input_operations().map(str::to_owned))
        .chain(
            calternal_core::REQUEST_OPERATIONS
                .iter()
                .map(|row| row.id.to_owned()),
        )
        .chain(
            calternal_server::authz::agent_policies()
                .iter()
                .map(|p| p.operation.to_string()),
        )
        .chain(
            calternal_server::authz::people_policies()
                .iter()
                .map(|p| p.operation.to_string()),
        )
        .chain(calternal_core::NOTE_OPERATIONS.iter().map(|op| {
            fixture
                .state
                .registry
                .snapshot()
                .policy(op.id)
                .unwrap()
                .operation
                .to_string()
        }))
        .chain(
            calternal_core::SearchOperation::ALL
                .iter()
                .map(|op| op.id().to_owned()),
        )
        .chain(
            calternal_core::AUTOMATION_OPERATIONS
                .iter()
                .map(|op| op.id.to_owned()),
        )
        .chain(
            calternal_server::authz::sharing_policies()
                .iter()
                .map(|p| p.operation.to_string()),
        )
        .chain(appearance.into_iter().map(str::to_owned))
        .chain(plugin_rows.iter().map(|p| (*p).to_owned()))
        .collect();
    assert_eq!(
        rows.iter()
            .map(|s| (*s).to_owned())
            .collect::<BTreeSet<_>>(),
        all
    );
    assert_eq!(
        POLICIES.len()
            + calternal_core::account_input_operations().count()
            + calternal_core::REQUEST_OPERATIONS.len()
            + calternal_server::authz::agent_policies().len()
            + calternal_server::authz::people_policies().len()
            + calternal_core::NOTE_OPERATIONS.len()
            + calternal_core::SearchOperation::ALL.len()
            + calternal_core::AUTOMATION_OPERATIONS.len()
            + calternal_server::authz::sharing_policies().len()
            + appearance.len()
            + plugin_rows.len(),
        MATRIX.len() + stored_rows.len(),
        "duplicate policy"
    );
    for &(op, _) in ONBOARDING_MATRIX {
        assert_eq!(catalog.policy(op).unwrap().scope, Scope::Account, "{op}");
    }
    // Threads require real durable jobs and protected authority. Their independent
    // Reader/Editor/Manager/Owner HTTP matrix is in conformance/threads_cases.rs.
    let threads = [
        ("threads.create", Capability::ContentWrite),
        ("threads.list", Capability::SpaceRead),
        ("threads.models", Capability::PluginsUse),
        ("threads.get", Capability::SpaceRead),
        ("threads.send", Capability::PluginsUse),
        ("threads.stop", Capability::PluginsUse),
        ("threads.archive", Capability::ContentWrite),
        ("threads.rename", Capability::ContentWrite),
        ("threads.snooze", Capability::ContentWrite),
        ("threads.unarchive", Capability::ContentWrite),
        ("threads.delete", Capability::ContentWrite),
    ];
    // The guide's real session and activation matrix is in conformance/plugin_slot_cases.rs.
    let guide = [
        ("calternal-plugin-guide.explain", Capability::SpaceRead),
        ("calternal-plugin-guide.scaffold", Capability::SpaceRead),
        ("calternal-plugin-guide.check", Capability::SpaceRead),
        ("calternal-plugin-guide.publish", Capability::ContentWrite),
    ];
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let registry = Registry::load(&root).snapshot();
    for plugin in registry.plugins() {
        for op in &plugin.manifest.operations {
            if let Some((_, capability)) = threads.iter().chain(&guide).find(|(id, _)| *id == op.id)
            {
                assert_eq!(
                    registry.policy(&op.id).unwrap().required,
                    *capability,
                    "{}",
                    op.id
                );
            } else {
                assert!(
                    rows.contains(op.id.as_str()),
                    "missing matrix row: {}",
                    op.id
                );
            }
        }
    }
}

/// Each operation accepts its capability alone and rejects every other capability.
#[tokio::test]
async fn every_operation_checks_only_its_required_capability() {
    for &(op, required, _) in MATRIX {
        for allowed in [true, false] {
            let fixture = Fixture::new(0, false).await;
            let grants = if allowed {
                CapabilitySet::new(&[required])
            } else {
                CapabilitySet::new(&Capability::ALL).without(required)
            };
            fixture.state.directory.set_capabilities(grants);
            let response = fixture
                .call(
                    Some(TOKEN),
                    "capability",
                    op,
                    fixture.input(op, &fixture.space()),
                )
                .await;
            if allowed {
                assert_eq!(response.0, StatusCode::OK, "{op}: {}", response.1);
            } else {
                denial(
                    response,
                    if item_operation(op) && required == Capability::SpaceRead {
                        StatusCode::NOT_FOUND
                    } else {
                        StatusCode::FORBIDDEN
                    },
                    if item_operation(op) && required == Capability::SpaceRead {
                        "item not found"
                    } else {
                        "insufficient role for this space"
                    },
                );
            }
        }
    }
}

#[test]
fn built_in_roles_cover_exact_documented_capabilities() {
    let names: BTreeSet<_> = Capability::ALL
        .iter()
        .map(|capability| capability.name())
        .collect();
    assert_eq!(names.len(), Capability::ALL.len());
    for capability in Capability::ALL {
        assert!(
            roles().iter().any(|role| role.contains(capability))
                || matches!(
                    capability,
                    Capability::SpacesList
                        | Capability::SpacesCreate
                        | Capability::PeopleManage
                        | Capability::GroupsManage
                        | Capability::AccountProfile
                        | Capability::InviteRedeem
                        | Capability::RolesList
                        | Capability::ServerSettings
                ),
            "unheld capability: {}",
            capability.name()
        );
        assert_eq!(Capability::from_name(capability.name()), Some(capability));
    }
    assert_eq!(Capability::from_name("unknown.permission"), None);
    assert_eq!(CapabilitySet::default(), CapabilitySet::empty());
    // Plugin rows come from the shipped manifests through the same lookup.
    let registry =
        Registry::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins")).snapshot();
    for (op, capability, _) in MATRIX {
        assert_eq!(registry.policy(op).unwrap().required, *capability);
    }
}
