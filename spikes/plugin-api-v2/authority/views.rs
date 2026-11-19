//! View-specific authority, delivery, leases and renderer selection.
use super::*;
use calternal_core::SharePermission;
use calternal_types::Capability;

impl Lease {
    fn wire(&self) -> Value {
        json!({"lease_ref":self.id,"lease_seconds":15,"grant_revision":self.grant,"cache_partition":self.partition})
    }
}

impl Host {
    pub(super) async fn current_item(
        &self,
        who: Identity,
        view: &View,
        id: Uuid,
        edit: bool,
    ) -> bool {
        calternal_server::resolve::authorize_item_access(
            &self.state,
            view.space,
            NOTES,
            id,
            who.id,
            if edit {
                SharePermission::Edit
            } else {
                SharePermission::View
            },
        )
        .await
        .is_ok()
    }

    async fn member(&self, who: Identity, view: &View) -> Option<calternal_types::CapabilitySet> {
        self.state
            .directory
            .protected_spaces()?
            .capabilities_async(who.id, view.space)
            .await
            .ok()?
    }

    pub(super) async fn editor(&self, who: Identity, view: &View) -> bool {
        self.member(who, view)
            .await
            .is_some_and(|caps| caps.contains(Capability::ContentWrite))
    }

    pub(super) async fn audience(&self, who: Identity, view: &View) -> Option<&'static str> {
        if self
            .member(who, view)
            .await
            .is_some_and(|caps| caps.contains(Capability::SpaceRead))
        {
            return Some(if view.home {
                "own_space_member"
            } else {
                "member"
            });
        }
        if view.home && self.current_item(who, view, view.rows[0], false).await {
            return Some("guest");
        }
        None
    }

    fn grant(&self, who: Identity, view: Uuid) -> Uuid {
        *self
            .live
            .lock()
            .unwrap()
            .grants
            .entry((who.id, view))
            .or_insert_with(Uuid::new_v4)
    }

    pub(crate) async fn sharing(&self, enable: bool) -> Option<Value> {
        let view = &self.views[&self.home];
        let _fence = view.gate.lock().await;
        if !enable {
            return None;
        }
        let grant=fixture::run(&self.state,fixture::identity(self.owner),"sharing.grant",
            json!({"item":{"kind":NOTES,"id":view.rows[0]},"audience":{"account_id":self.viewer},"permission":"view"})).await.unwrap();
        self.live
            .lock()
            .unwrap()
            .grants
            .remove(&(self.viewer, self.home));
        Some(grant["grant_id"].clone())
    }

    pub(crate) async fn unshare(&self, grant: Value) {
        let view = &self.views[&self.home];
        let _fence = view.gate.lock().await;
        fixture::run(
            &self.state,
            fixture::identity(self.owner),
            "sharing.revoke",
            json!({"item":{"kind":NOTES,"id":view.rows[0]},"grant_id":grant}),
        )
        .await
        .unwrap();
        self.live
            .lock()
            .unwrap()
            .grants
            .remove(&(self.viewer, self.home));
    }

    pub(crate) async fn member_role(&self, role: Option<&str>) {
        let view = &self.views[&self.shared];
        let _fence = view.gate.lock().await;
        let owner = fixture::identity(self.owner);
        if role.is_none() {
            fixture::run(
                &self.state,
                owner,
                "spaces.remove_member",
                json!({"space_id":view.space,"account_id":self.viewer}),
            )
            .await
            .unwrap();
        } else {
            let auth = self.state.directory.protected_spaces().unwrap();
            if auth
                .capabilities_async(self.viewer, view.space)
                .await
                .unwrap()
                .is_some()
            {
                let role = role.unwrap();
                tokio::task::block_in_place(|| {
                    auth.set_role(self.owner, view.space, self.viewer, role)
                })
                .unwrap();
            } else {
                let path = self.state.store.appdata().join("system/auth.db");
                let viewer = self.viewer.to_string();
                let space = view.space.to_string();
                let role = role.unwrap().to_owned();
                // Restore only the disposable initial fixture membership.
                tokio::task::spawn_blocking(move || {
                    rusqlite::Connection::open(path)
                        .unwrap()
                        .execute(
                            "INSERT INTO space_memberships VALUES(?1,?2,?3)",
                            [space, viewer, role],
                        )
                        .unwrap();
                })
                .await
                .unwrap();
            }
        }
        self.live
            .lock()
            .unwrap()
            .grants
            .remove(&(self.viewer, self.shared));
    }

    fn mint(&self, lease: &Lease, row: Option<(Uuid, String)>, actions: &[Action]) -> Value {
        self.mint_in(&mut self.live.lock().unwrap(), lease, row, actions)
    }

    pub(super) fn mint_in(
        &self,
        state: &mut Volatile,
        lease: &Lease,
        row: Option<(Uuid, String)>,
        actions: &[Action],
    ) -> Value {
        let mut result = serde_json::Map::new();
        let spaces = state
            .leases
            .iter()
            .filter(|(_, p)| self.views[&p.view].space == self.views[&lease.view].space)
            .map(|(id, _)| *id)
            .collect::<std::collections::HashSet<_>>();
        let person = state
            .leases
            .iter()
            .filter(|(_, p)| p.actor == lease.actor)
            .map(|(id, _)| *id)
            .collect::<std::collections::HashSet<_>>();
        let count = |state: &Volatile, leases: &std::collections::HashSet<_>| {
            state
                .contexts
                .values()
                .filter(|c| leases.contains(&c.lease))
                .count()
        };
        let existing = |action: Action| {
            state.contexts.iter().find_map(|(id, prior)| {
                (prior.lease == lease.id
                    && prior.row == row.as_ref().map(|r| r.0)
                    && prior.revision.as_ref() == row.as_ref().map(|r| &r.1)
                    && prior.action == action)
                    .then_some(*id)
            })
        };
        let issued: Vec<_> = actions
            .iter()
            .map(|action| (*action, existing(*action)))
            .collect();
        let needed = issued.iter().filter(|(_, id)| id.is_none()).count();
        if count(state, &spaces) + needed > 4096 || count(state, &person) + needed > 256 {
            return json!({});
        }
        for (action, id) in issued {
            let c = Context {
                lease: lease.id,
                row: row.as_ref().map(|r| r.0),
                revision: row.as_ref().map(|r| r.1.clone()),
                action,
            };
            let id = id.unwrap_or_else(Uuid::new_v4);
            state.contexts.insert(id, c);
            result.insert(action.name().into(), json!(id));
        }
        Value::Object(result)
    }

    pub(super) async fn open_view(&self, who: Identity, session: Uuid, id: Uuid) -> Reply<Value> {
        let view = self.views.get(&id).ok_or(HIDDEN)?;
        let _fence = view.gate.lock().await;
        let audience = self.audience(who, view).await.ok_or(HIDDEN)?;
        let lease = Lease {
            id: Uuid::new_v4(),
            view: id,
            actor: who.id,
            session,
            partition: Uuid::new_v4(),
            grant: self.grant(who, id),
            audience,
            deadline: Instant::now() + LIFETIME,
        };
        self.live
            .lock()
            .unwrap()
            .leases
            .insert(lease.id, lease.clone());
        let contexts = if self.editor(who, view).await {
            match view.display.as_ref().map(|d| d.app) {
                Some(App::Reading) => self.mint(&lease, None, &[Action::Add]),
                Some(app) => self.mint(&lease, None, display_actions::view_actions(app)),
                None => self.mint(&lease, None, &[Action::Bulk]),
            }
        } else {
            json!({})
        };
        Ok(json!({"lease":lease.wire(),"activation_contexts":contexts,"audience":audience}))
    }

    pub(super) async fn lease(
        &self,
        who: Identity,
        session: Uuid,
        id: Uuid,
    ) -> Reply<(Lease, View)> {
        let lease = self
            .live
            .lock()
            .unwrap()
            .leases
            .get(&id)
            .cloned()
            .ok_or(HIDDEN)?;
        if lease.actor != who.id || lease.session != session || lease.deadline <= Instant::now() {
            return Err(HIDDEN);
        }
        let view = self.views[&lease.view].clone();
        if self.audience(who, &view).await != Some(lease.audience)
            || self.grant(who, lease.view) != lease.grant
        {
            return Err(HIDDEN);
        }
        Ok((lease, view))
    }

    pub(super) async fn current_head(&self, view: &View, id: Uuid) -> Reply<String> {
        if let Some(display) = &view.display {
            return display.revision(id);
        }
        let state = self.state.clone();
        let space = view.space;
        tokio::task::spawn_blocking(move || state.store.content().live_revision(space, NOTES, id))
            .await
            .unwrap()
            .map_err(|_| HIDDEN)
    }

    pub(super) async fn window(
        &self,
        who: Identity,
        session: Uuid,
        id: Uuid,
        input: &Value,
        route: &DisplayRoute,
    ) -> Reply<Value> {
        let (lease, view) = self.lease(who, session, id).await?;
        if let Some(display) = &view.display {
            return self
                .display_window(who, &lease, &view, display, input, route)
                .await;
        }
        if input["bind"] != NOTES || input.get("parent").is_some() {
            return Err(HIDDEN);
        }
        let from = input["from"].as_u64().ok_or(INVALID)?;
        let to = input["to"].as_u64().ok_or(INVALID)?;
        let end = input["anchor"] == "end";
        if from >= to || to - from > 200 || (!end && from > 500_000) {
            return Err(INVALID);
        }
        if end && lease.audience == "guest" {
            return Ok(
                json!({"status":422,"code":"unsupported_query_plan","reason":"end_unavailable"}),
            );
        }
        let write = self.editor(who, &view).await;
        let mut visible = Vec::new();
        for &item in &view.rows {
            if (lease.audience != "guest" || item == view.rows[0])
                && self.current_item(who, &view, item, false).await
            {
                visible.push(item);
            }
        }
        let total = visible.len() as u64;
        let (start, stop) = if end {
            (total.saturating_sub(to), total.saturating_sub(from))
        } else {
            (from.min(total), to.min(total))
        };
        let mut rows = Vec::new();
        let mut recorded = Vec::new();
        for &id in &visible[start as usize..stop as usize] {
            let state = self.state.clone();
            let space = view.space;
            let item = tokio::task::spawn_blocking(move || state.store.content().get(space, id))
                .await
                .unwrap()
                .map_err(|_| HIDDEN)?;
            let mut fields = json!({"title":item.metadata["title"]});
            if lease.audience != "guest" {
                fields["updated"] = json!(item.updated_at);
                fields["pinned"] = json!(item.metadata["pinned"].as_bool().unwrap_or(false));
            }
            recorded.push((id, item.revision));
            rows.push(json!({"id":id,"fields":fields}));
        }
        self.hooks.hit(1).await;
        let _fence = view.gate.lock().await;
        self.lease(who, session, id).await?;
        for (item, revision) in &recorded {
            if self.current_head(&view, *item).await? != *revision
                || !self.current_item(who, &view, *item, false).await
            {
                return Err(STALE);
            }
        }
        let reference = Uuid::new_v4();
        let version = Uuid::new_v4();
        let mut actions = Vec::new();
        if write && self.editor(who, &view).await {
            actions=recorded.iter().map(|row|json!({"row":row.0,
                "contexts":self.mint(&lease,Some(row.clone()),&[Action::Pin,Action::Prefill,Action::Choice,Action::Ordinary])})).collect();
        }
        {
            let mut state = self.live.lock().unwrap();
            state.window_order += 1;
            let order = state.window_order;
            state.windows.insert(
                reference,
                IssuedWindow {
                    lease: id,
                    version,
                    order,
                    rows: recorded,
                    display: None,
                },
            );
        }
        let mut out = json!({"bind":NOTES,"state":if total==0 && from==0{"empty"}else{"ready"},
            "result_version":version,"offset":from,"rows":rows,"window_ref":reference,"audience":lease.audience,
            "activation_contexts":actions});
        if end {
            out["anchor"] = json!("end");
        }
        if lease.audience != "guest" {
            out["count"] = json!({"mode":"exact","value":total});
        }
        Ok(out)
    }

    pub(super) async fn selection(
        &self,
        who: Identity,
        session: Uuid,
        id: Uuid,
        input: &Value,
    ) -> Reply<Value> {
        let (lease, view) = self.lease(who, session, id).await?;
        let _fence = view.gate.lock().await;
        self.lease(who, session, id).await?;
        if let Some(display) = &view.display {
            return self.display_selection(&lease, display, input, self.editor(who, &view).await);
        }
        if input["bind"] != NOTES {
            return Err(HIDDEN);
        }
        let row = ref_at(input, "row")?;
        let head = self.issued_row(id, row).ok_or(HIDDEN)?;
        if self.current_head(&view, row).await? != head
            || !self.current_item(who, &view, row, false).await
        {
            return Err(STALE);
        }
        let contexts = if self.editor(who, &view).await {
            self.mint(
                &lease,
                Some((row, head)),
                &[
                    Action::Pin,
                    Action::Prefill,
                    Action::Choice,
                    Action::Ordinary,
                ],
            )
        } else {
            json!({})
        };
        Ok(
            json!({"type":"selection_context","lease_ref":lease.id,"cache_partition":lease.partition,
            "bind":NOTES,"row":row,"activation_contexts":contexts}),
        )
    }

    pub(super) fn issued_row(&self, lease: Uuid, id: Uuid) -> Option<String> {
        self.live
            .lock()
            .unwrap()
            .windows
            .values()
            .filter(|w| w.lease == lease)
            .filter_map(|w| {
                w.rows
                    .iter()
                    .find(|r| r.0 == id)
                    .map(|r| (w.order, r.1.clone()))
            })
            .max_by_key(|r| r.0)
            .map(|r| r.1)
    }

    pub(super) async fn renew(&self, who: Identity, session: Uuid, input: &Value) -> Reply<Value> {
        let id = ref_at(input, "lease_ref")?;
        let (mut lease, view) = self.lease(who, session, id).await?;
        if ref_at(input, "cache_partition")? != lease.partition
            || ref_at(input, "grant_revision")? != lease.grant
        {
            return Err(HIDDEN);
        }
        let _fence = view.gate.lock().await;
        self.lease(who, session, id).await?;
        let candidates = {
            let live = self.live.lock().unwrap();
            input["result_versions"]
                .as_object()
                .unwrap()
                .iter()
                .filter_map(|(key, v)| {
                    let id = Uuid::parse_str(key).ok()?;
                    let w = live.windows.get(&id)?;
                    (json!(w.version) == *v && w.lease == lease.id).then(|| (id, w.clone()))
                })
                .collect::<Vec<_>>()
        };
        let mut kept = Vec::new();
        for (id, window) in candidates {
            if let Some(display) = &view.display {
                if window
                    .display
                    .as_ref()
                    .is_some_and(|(_, generation)| *generation == display.generation())
                {
                    kept.push(json!({"window_ref":id,"result_version":window.version}));
                }
                continue;
            }
            let mut current = true;
            for (row, revision) in &window.rows {
                if self.current_head(&view, *row).await.ok().as_ref() != Some(revision)
                    || !self.current_item(who, &view, *row, false).await
                {
                    current = false;
                    break;
                }
            }
            if current {
                kept.push(json!({"window_ref":id,"result_version":window.version}));
            }
        }
        lease.deadline = Instant::now() + LIFETIME;
        self.live
            .lock()
            .unwrap()
            .leases
            .insert(lease.id, lease.clone());
        let actions = if self.editor(who, &view).await {
            match view.display.as_ref().map(|d| d.app) {
                Some(app) => display_actions::view_actions(app),
                None => &[Action::Bulk],
            }
        } else {
            &[]
        };
        let contexts = self.mint(&lease, None, actions);
        Ok(
            json!({"lease":lease.wire(),"retained_results":kept,"activation_contexts":contexts,"windows":[],
            "renewal_id":input["renewal_id"],"request_lease_ref":id,"cache_partition":lease.partition}),
        )
    }
}
