//! Independent operation inventory and capability declarations, separate from role/race cases.
use super::*;

#[tokio::test]
async fn every_registered_operation_has_an_independent_matrix_row() {
    let fixture = Fixture::new(3, false).await;
    let rows: BTreeSet<_> = MATRIX.iter().map(|(op, _, _)| *op).collect();
    let mut registered: BTreeSet<_> = operation_catalog().into_iter().map(|op| op.id).collect();
    for plugin in fixture.state.registry.plugins() {
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
        "notes.lookup",
        "tags.list",
        "tags.lookup",
        "trash.list",
        "trash.purge",
        "resolve", // dedicated address matrix in resolver.rs
        "spaces.list",
        "spaces.create",
        "spaces.members",
        "spaces.set_role",
        "spaces.remove_member",
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
    // Manual-turn controls are covered by provider_turns.rs and agent_cases conformance.
    stored_rows.extend(
        calternal_core::AGENT_OPERATIONS
            .iter()
            .map(|(id, _, _)| *id),
    );
    // Request isolation and the instructions role matrix use real passkey
    // sessions in conformance/requests_cases.rs.
    for (id, required) in [
        ("requests.list", Capability::SpaceRead),
        ("requests.ask", Capability::PluginsUse),
        ("requests.answer", Capability::SpaceRead),
        ("requests.approve", Capability::PluginsUse),
        ("requests.deny", Capability::PluginsUse),
        ("space.instructions", Capability::SpaceSettings),
    ] {
        assert!(stored_rows.insert(id), "duplicate request operation {id}");
        assert_eq!(
            fixture.state.registry.policy(id).unwrap().required,
            required
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
    let plugin_rows: Vec<&str> = fixture
        .state
        .registry
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
        .chain(calternal_core::NOTE_OPERATIONS.iter().map(|op| {
            fixture
                .state
                .registry
                .policy(op.id)
                .unwrap()
                .operation
                .to_string()
        }))
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
            + calternal_core::NOTE_OPERATIONS.len()
            + plugin_rows.len(),
        MATRIX.len() + stored_rows.len(),
        "duplicate policy"
    );
    // Threads require real durable jobs and protected authority. Their independent
    // Reader/Editor/Manager/Owner HTTP matrix is in conformance/threads_cases.rs.
    let threads = [
        ("threads.create", Capability::ContentWrite),
        ("threads.list", Capability::SpaceRead),
        ("threads.get", Capability::SpaceRead),
        ("threads.send", Capability::PluginsUse),
        ("threads.stop", Capability::PluginsUse),
        ("threads.archive", Capability::ContentWrite),
        ("threads.rename", Capability::ContentWrite),
        ("threads.unarchive", Capability::ContentWrite),
        ("threads.delete", Capability::ContentWrite),
    ];
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let registry = Registry::load(&root);
    for plugin in registry.plugins() {
        for op in &plugin.manifest.operations {
            if let Some((_, capability)) = threads.iter().find(|(id, _)| *id == op.id) {
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
                    Capability::SpacesList | Capability::SpacesCreate
                ),
            "unheld capability: {}",
            capability.name()
        );
        assert_eq!(Capability::from_name(capability.name()), Some(capability));
    }
    assert_eq!(Capability::from_name("unknown.permission"), None);
    assert_eq!(CapabilitySet::default(), CapabilitySet::empty());
    // Plugin rows come from the shipped manifests through the same lookup.
    let registry = Registry::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins"));
    for (op, capability, _) in MATRIX {
        assert_eq!(registry.policy(op).unwrap().required, *capability);
    }
}
