//! Resolve only issued references and hold the space fence through the commit.
use super::*;
use calternal_content::Request;
use calternal_core::SharePermission;
use calternal_store::{Commit, IdemWrite, StampedEvent};
use std::collections::HashSet;

impl Host {
    async fn context(
        &self,
        who: Identity,
        session: Uuid,
        key: Uuid,
    ) -> Reply<(Context, Lease, View)> {
        let context = self
            .live
            .lock()
            .unwrap()
            .contexts
            .get(&key)
            .cloned()
            .ok_or(HIDDEN)?;
        let (lease, view) = self.lease(who, session, context.lease).await?;
        if !self.editor(who, &view).await {
            return Err(FORBIDDEN);
        }
        Ok((context, lease, view))
    }

    async fn collection_current(&self, who: Identity, view: &View, id: Uuid) -> bool {
        calternal_server::resolve::authorize_item_access(
            &self.state,
            view.space,
            "collections",
            id,
            who.id,
            SharePermission::Edit,
        )
        .await
        .is_ok()
    }

    async fn collection_revision(&self, view: &View, id: Uuid) -> Reply<String> {
        let state = self.state.clone();
        let space = view.space;
        tokio::task::spawn_blocking(move || {
            state
                .store
                .content()
                .live_revision(space, "collections", id)
        })
        .await
        .unwrap()
        .map_err(|_| HIDDEN)
    }

    pub(super) async fn choices(
        &self,
        who: Identity,
        session: Uuid,
        input: &Value,
    ) -> Reply<Value> {
        let key = ref_at(input, "activation_context")?;
        let (context, _, view) = self.context(who, session, key).await?;
        let _fence = view.gate.lock().await;
        self.context(who, session, key).await?;
        if let Action::Display(_, effect) = context.action {
            let (slot, bind) = effect.choice().ok_or(HIDDEN)?;
            if input["slot"] != slot {
                return Err(HIDDEN);
            }
            let display = view.display.as_ref().ok_or(HIDDEN)?;
            if let Some(row) = context.row {
                self.check_targets(
                    who,
                    &view,
                    &[(row, context.revision.clone().ok_or(HIDDEN)?)],
                )
                .await?;
            }
            let items = display.options(who.id, bind);
            let mut live = self.live.lock().unwrap();
            live.choices.retain(|_, choice| choice.context != key);
            let out = items
                .into_iter()
                .map(|(destination, label, revision)| {
                    let token = Uuid::new_v4();
                    let reply = json!({"ref":token,"label":label});
                    live.choices.insert(
                        token,
                        Choice {
                            context: key,
                            destination,
                            revision,
                            label,
                        },
                    );
                    reply
                })
                .collect::<Vec<_>>();
            return Ok(json!({"bind":bind,"options":out}));
        }
        if context.action != Action::Choice || input["slot"] != CHOICE {
            return Err(HIDDEN);
        }
        let mut out = Vec::new();
        for (id, label) in &view.options {
            if !self.collection_current(who, &view, *id).await {
                continue;
            }
            let revision = self.collection_revision(&view, *id).await?;
            let token = Uuid::new_v4();
            self.live.lock().unwrap().choices.insert(
                token,
                Choice {
                    context: key,
                    destination: *id,
                    revision,
                    label: label.clone(),
                },
            );
            out.push(json!({"ref":token,"label":label}));
        }
        Ok(json!({"bind":"collections","options":out}))
    }

    pub(super) async fn resolve(
        &self,
        who: Identity,
        session: Uuid,
        input: &Value,
    ) -> Reply<(&'static str, Value)> {
        let key = ref_at(input, "activation_context")?;
        let activation = ref_at(input, "activation_id")?;
        let (context, lease, view) = self.context(who, session, key).await?;
        if input["action_id"] != context.action.name() {
            return Err(HIDDEN);
        }
        let interaction = input
            .get("interaction")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut targets = Vec::new();
        let mut material = Vec::new();
        let mut selection = None;
        let mut destination = None;
        let fence = view.gate.lock().await;
        self.context(who, session, key).await?;
        let prior = {
            let live = self.live.lock().unwrap();
            live.activations
                .get(&(who.id, activation))
                .and_then(|id| live.resolutions.get(id).map(|r| (*id, r.clone())))
        };
        if let Some((id, prior)) = prior {
            if prior.context != key || prior.interaction != interaction {
                return Err(CONFLICT);
            }
            drop(fence);
            return self
                .resolution_reply(who, session, id, activation, context.action, &prior, input)
                .await;
        }
        if let Some(row) = context.row {
            targets.push((row, context.revision.clone().ok_or(HIDDEN)?));
        }
        let fields = interaction.as_object().ok_or(INVALID)?;
        match context.action {
            Action::Display(_, effect) => {
                use display_actions::Effect;
                let display = view.display.as_ref().ok_or(HIDDEN)?;
                if let Some((slot, _)) = effect.choice() {
                    if fields.len() != 1
                        || interaction["choices"]
                            .as_object()
                            .is_none_or(|c| c.len() != 1)
                    {
                        return Err(INVALID);
                    }
                    let token = ref_at(&interaction["choices"], slot)?;
                    let picked = self
                        .live
                        .lock()
                        .unwrap()
                        .choices
                        .get(&token)
                        .cloned()
                        .ok_or(HIDDEN)?;
                    if picked.context != key {
                        return Err(HIDDEN);
                    }
                    if display.destination(who.id, picked.destination)? != picked.revision {
                        return Err(STALE);
                    }
                    destination = Some((picked.destination, picked.revision));
                    material.push(
                        json!({"name":"destination","label":"Destination","value":picked.label}),
                    );
                } else if matches!(
                    effect,
                    Effect::Rename | Effect::RenameSection | Effect::CreateSection
                ) {
                    if fields.len() > 1 || fields.keys().any(|k| k != "prompt" && k != "form") {
                        return Err(INVALID);
                    }
                    let prompt = interaction
                        .get("prompt")
                        .or_else(|| interaction.get("form"));
                    if prompt.is_some_and(|p| {
                        p.as_object().is_none_or(|p| p.keys().any(|k| k != "title"))
                    }) {
                        return Err(INVALID);
                    }
                    let title = match prompt.and_then(|p| p.get("title")) {
                        Some(value) => value.as_str().ok_or(INVALID)?.to_owned(),
                        None if effect == Effect::Rename => {
                            display.title(context.row.ok_or(HIDDEN)?)?
                        }
                        None => return Err(INVALID),
                    };
                    if title.trim().is_empty() || title.len() > 256 {
                        return Err(INVALID);
                    }
                    material.push(json!({"name":"title","label":"New title","value":title}));
                } else if effect == Effect::BulkTrash {
                    if fields.len() != 1 {
                        return Err(INVALID);
                    }
                    if interaction.get("selection_ref").is_some() {
                        let token = ref_at(&interaction, "selection_ref")?;
                        let live = self.live.lock().unwrap();
                        let (_, rows) = live
                            .display_sets
                            .get(&token)
                            .filter(|(id, _)| *id == lease.id)
                            .ok_or(HIDDEN)?;
                        targets.extend(rows.iter().cloned());
                        selection = Some(token);
                    } else {
                        for value in interaction["rows"].as_array().ok_or(INVALID)? {
                            let row = value
                                .as_str()
                                .and_then(|s| Uuid::parse_str(s).ok())
                                .ok_or(HIDDEN)?;
                            targets.push((
                                row,
                                self.display_row(lease.id, row, "notes").ok_or(HIDDEN)?,
                            ));
                        }
                    }
                    selection.get_or_insert_with(Uuid::new_v4);
                    if targets.is_empty() {
                        return Err(INVALID);
                    }
                    material.push(
                        json!({"name":"selection","label":"Selected notes","value":targets.len()}),
                    );
                } else if !fields.is_empty() {
                    return Err(INVALID);
                }
            }
            Action::Add => {
                if view.display.as_ref().is_none_or(|d| d.app != App::Reading) {
                    return Err(HIDDEN);
                }
                super::display::reading_input(&interaction)?;
            }
            Action::Pin | Action::Ordinary => {
                if !fields.is_empty() {
                    return Err(INVALID);
                }
            }
            Action::Prefill => {
                if fields.len() > 1 || fields.keys().any(|k| k != "form" && k != "prompt") {
                    return Err(INVALID);
                }
                let input = interaction
                    .get("form")
                    .or_else(|| interaction.get("prompt"));
                let form = input.and_then(Value::as_object);
                if input.is_some() && form.is_none() {
                    return Err(INVALID);
                }
                if form.is_some_and(|f| f.keys().any(|k| k != "title")) {
                    return Err(INVALID);
                }
                let title = match form.and_then(|f| f.get("title")) {
                    Some(value) => value.as_str().ok_or(INVALID)?,
                    None => "Untouched fixture prefill",
                };
                if title.trim().is_empty() || title.len() > 256 {
                    return Err(INVALID);
                }
                material.push(json!({"name":"title","label":"New title","value":title}));
            }
            Action::Choice => {
                if fields.len() != 1
                    || !fields.contains_key("choices")
                    || interaction["choices"]
                        .as_object()
                        .is_none_or(|o| o.len() != 1)
                {
                    return Err(INVALID);
                }
                let choice = ref_at(&interaction["choices"], CHOICE)?;
                let picked = self
                    .live
                    .lock()
                    .unwrap()
                    .choices
                    .get(&choice)
                    .cloned()
                    .ok_or(HIDDEN)?;
                if picked.context != key
                    || !self
                        .collection_current(who, &view, picked.destination)
                        .await
                {
                    return Err(HIDDEN);
                }
                if self.collection_revision(&view, picked.destination).await? != picked.revision {
                    return Err(STALE);
                }
                destination = Some((picked.destination, picked.revision));
                material
                    .push(json!({"name":"destination","label":"Destination","value":picked.label}));
            }
            Action::Bulk => {
                if fields.len() != 1 || !fields.contains_key("rows") {
                    return Err(INVALID);
                }
                let rows = interaction["rows"]
                    .as_array()
                    .filter(|r| !r.is_empty())
                    .ok_or(INVALID)?;
                let mut unique = HashSet::new();
                for row in rows {
                    let row = row
                        .as_str()
                        .and_then(|s| Uuid::parse_str(s).ok())
                        .ok_or(HIDDEN)?;
                    if !unique.insert(row) {
                        return Err(INVALID);
                    }
                    targets.push((row, self.issued_row(lease.id, row).ok_or(HIDDEN)?));
                }
                material.push(json!({"name":"selection","label":"Selected notes","value":targets.iter().map(|t|t.0).collect::<Vec<_>>()}));
                selection = Some(Uuid::new_v4());
            }
        }
        self.check_targets(who, &view, &targets).await?;
        let id = Uuid::new_v4();
        let record = Resolution {
            context: key,
            activation,
            interaction,
            targets,
            material,
            selection,
            destination,
            challenge: context.action.challenge().then(Uuid::new_v4),
            proof: None,
        };
        {
            let mut live = self.live.lock().unwrap();
            live.resolutions.insert(id, record.clone());
            live.activations.insert((who.id, activation), id);
        }
        drop(fence);
        self.resolution_reply(who, session, id, activation, context.action, &record, input)
            .await
    }

    async fn resolution_reply(
        &self,
        who: Identity,
        session: Uuid,
        id: Uuid,
        activation: Uuid,
        action: Action,
        record: &Resolution,
        input: &Value,
    ) -> Reply<(&'static str, Value)> {
        let atomic = matches!(
            action,
            Action::Pin | Action::Display(_, display_actions::Effect::Pin)
        );
        if input["dispatch"] == true && atomic {
            return Ok((
                "outcome",
                self.dispatch(
                    who,
                    session,
                    &json!({
                        "type":"invoke_dispatch","resolution_ref":id,"activation_id":activation
                    }),
                )
                .await?,
            ));
        }
        let mut out = json!({"activation_id":activation,"resolution_ref":id});
        if let Some(challenge) = record.challenge {
            out["confirm"] = json!("challenge");
            out["confirmation_challenge"] = json!(challenge);
            out["material"] = json!(record.material);
        } else if atomic {
            out["confirm"] = json!("none");
        }
        if let Some(selection) = record.selection {
            out["selection_ref"] = json!(selection);
        }
        Ok(("resolution", out))
    }

    fn resolution(&self, input: &Value) -> Reply<(Uuid, Resolution)> {
        let id = ref_at(input, "resolution_ref")?;
        let value = self
            .live
            .lock()
            .unwrap()
            .resolutions
            .get(&id)
            .cloned()
            .ok_or(HIDDEN)?;
        if value.activation != ref_at(input, "activation_id")? {
            return Err(HIDDEN);
        }
        Ok((id, value))
    }

    pub(super) async fn check_targets(
        &self,
        who: Identity,
        view: &View,
        targets: &[(Uuid, String)],
    ) -> Reply<()> {
        for (row, rev) in targets {
            if view.display.is_none() && !self.current_item(who, view, *row, true).await {
                return Err(HIDDEN);
            }
            if self.current_head(view, *row).await? != *rev {
                return Err(STALE);
            }
        }
        Ok(())
    }

    pub(super) async fn confirm(
        &self,
        who: Identity,
        session: Uuid,
        input: &Value,
    ) -> Reply<Value> {
        let (id, r) = self.resolution(input)?;
        let (_, _, view) = self.context(who, session, r.context).await?;
        self.hooks.hit(3).await;
        let _fence = view.gate.lock().await;
        self.context(who, session, r.context).await?;
        if r.challenge != Some(ref_at(input, "confirmation_challenge")?) {
            return Err(HIDDEN);
        }
        if input.get("final_text").is_some() {
            return Err(INVALID);
        }
        self.check_targets(who, &view, &r.targets).await?;
        if let Some((destination, revision)) = &r.destination {
            let current = if let Some(display) = &view.display {
                display.destination(who.id, *destination)?
            } else {
                if !self.collection_current(who, &view, *destination).await {
                    return Err(HIDDEN);
                }
                self.collection_revision(&view, *destination).await?
            };
            if current != *revision {
                return Err(STALE);
            }
        }
        let mut live = self.live.lock().unwrap();
        let entry = live.resolutions.get_mut(&id).ok_or(HIDDEN)?;
        let proof = *entry.proof.get_or_insert_with(Uuid::new_v4);
        Ok(json!({"confirmation_proof":proof}))
    }

    pub(super) async fn dispatch(
        &self,
        who: Identity,
        session: Uuid,
        input: &Value,
    ) -> Reply<Value> {
        let (_, r) = self.resolution(input)?;
        let (ctx, _, view) = self.context(who, session, r.context).await?;
        self.hooks.hit(2).await;
        let _fence = view.gate.lock().await;
        self.context(who, session, r.context).await?;
        let (_, r) = self.resolution(input)?;
        let supplied = input
            .get("confirmation_proof")
            .map(|_| ref_at(input, "confirmation_proof"))
            .transpose()?;
        if let Some(_) = r.challenge {
            if supplied.is_none() || r.proof != supplied {
                return Err(FORBIDDEN);
            }
        } else if supplied.is_some() {
            return Err(INVALID);
        }
        if let Action::Display(_, effect) = ctx.action {
            let display = view.display.as_ref().ok_or(HIDDEN)?;
            return display.apply(
                who.id,
                r.activation,
                effect,
                &r,
                &self.undo_ref(who.id, view.space, r.activation),
            );
        }
        if ctx.action == Action::Add {
            let display = view
                .display
                .as_ref()
                .filter(|d| d.app == App::Reading)
                .ok_or(HIDDEN)?;
            return display.add(who.id, r.activation, &r.interaction);
        }
        let request_key = format!("step0/{}", r.activation);
        let current = {
            let state = self.state.clone();
            let key = request_key.clone();
            let space = view.space;
            tokio::task::spawn_blocking(move || state.store.content().receipt(space, who.id, &key))
                .await
                .unwrap()
                .map_err(|_| STALE)?
        };
        // Replays still recheck current item authority, even when their original
        // content revision advanced as a result of the already-committed edit.
        for (item, _) in &r.targets {
            if !self.current_item(who, &view, *item, true).await {
                return Err(HIDDEN);
            }
        }
        if let Some((destination, _)) = &r.destination {
            if !self.collection_current(who, &view, *destination).await {
                return Err(HIDDEN);
            }
        }
        if let Some(receipt) = current {
            if receipt.operation != ctx.action.name() {
                return Err(CONFLICT);
            }
            return Ok(if ctx.action == Action::Choice {
                json!({"activation_id":r.activation,"state":"ok"})
            } else {
                receipt.result["outcome"].clone()
            });
        }
        self.check_targets(who, &view, &r.targets).await?;
        if ctx.action == Action::Choice {
            return self.move_collection(who, &view, &r, request_key).await;
        }
        if ctx.action == Action::Bulk {
            return self
                .record_set_effect(who, &view, ctx.action, &r, request_key)
                .await;
        }
        self.edit(who, &view, ctx.action, &r, request_key).await
    }

    async fn move_collection(
        &self,
        who: Identity,
        view: &View,
        r: &Resolution,
        request_key: String,
    ) -> Reply<Value> {
        let (id, revision) = r.destination.clone().ok_or(HIDDEN)?;
        let row = r.targets.first().ok_or(HIDDEN)?.0;
        let state = self.state.clone();
        let space = view.space;
        let activation = r.activation;
        tokio::task::spawn_blocking(move || {
            let content = state.store.content();
            let collection = content.collection(space, id).map_err(|_| HIDDEN)?;
            if collection.revision != revision {
                return Err(STALE);
            }
            let mut members = collection.collection.members;
            if !members.contains(&row) {
                members.push(row);
            }
            content
                .set_collection_members(
                    space,
                    id,
                    &revision,
                    members,
                    Request {
                        account: who.id,
                        via: None,
                        id: &request_key,
                        operation: Action::Choice.name(),
                        fingerprint: &request_key,
                    },
                )
                .map_err(|_| STALE)?;
            Ok(json!({"activation_id":activation,"state":"ok"}))
        })
        .await
        .unwrap()
    }

    async fn record_set_effect(
        &self,
        who: Identity,
        view: &View,
        action: Action,
        r: &Resolution,
        request_key: String,
    ) -> Reply<Value> {
        // The Step 0 scoped set executor writes one durable fixture action.
        // The product multi-item Trash writer is outside this fixture.
        let state = self.state.clone();
        let space = view.space;
        let log = tokio::task::spawn_blocking(move || state.store.content().live_journal(space))
            .await
            .unwrap()
            .map_err(|_| STALE)?;
        if let Some(prior) = log
            .idempotency(who.id, &request_key)
            .await
            .map_err(|_| STALE)?
        {
            return Ok(prior.result["payload"]["outcome"].clone());
        }
        let outcome = json!({"activation_id":r.activation,"state":"ok"});
        let data = json!({"targets":r.targets,"material":r.material,"destination":r.destination});
        let event = StampedEvent {
            author: who.id,
            kind: "spike.activation".into(),
            item: r.activation.to_string(),
            data,
            installation: None,
            request_id: Some(request_key.clone()),
        };
        log.commit(Commit {
            events: vec![event],
            durable: true,
            idempotency: Some(IdemWrite {
                account: who.id,
                key: request_key.clone(),
                action: action.name().into(),
                payload_hash: request_key,
                result: json!({"payload":{"outcome":outcome}}),
            }),
        })
        .await
        .map_err(|_| STALE)?;
        Ok(outcome)
    }

    async fn edit(
        &self,
        who: Identity,
        view: &View,
        action: Action,
        r: &Resolution,
        request_key: String,
    ) -> Reply<Value> {
        let (row, rev) = r.targets.first().cloned().ok_or(HIDDEN)?;
        let state = self.state.clone();
        let space = view.space;
        let material = r.material.clone();
        let activation = r.activation;
        let undo_token = self.undo_ref(who.id, space, activation);
        tokio::task::spawn_blocking(move || {
            let content=state.store.content();
            let before=content.get(space,row).map_err(|_|HIDDEN)?;
            if before.revision!=rev {return Err(STALE);}
            let mut metadata=before.metadata.clone();
            let mut outcome=json!({"activation_id":activation,"state":"ok"});
            let mut inverse=Value::Null;
            match action {
                Action::Pin=>{
                    inverse=json!({"pinned":before.metadata["pinned"]});
                    metadata["pinned"]=json!(true);
                    outcome=super::undo::pin_outcome(activation,&undo_token);
                }
                Action::Prefill=>metadata["title"]=material[0]["value"].clone(),
                Action::Ordinary=>metadata["spike_reminded"]=json!(true),
                _=>unreachable!(),
            }
            let request=Request {account:who.id,via:None,id:&request_key,operation:action.name(),fingerprint:&request_key};
            content.write(space,Some(row),NOTES,Some(&rev),metadata,&before.bytes,request,
                |after|json!({"outcome":outcome,"target":row,"post_revision":after.revision,"inverse":inverse}))
                .map_err(|_|STALE)?;
            Ok(outcome)
        }).await.unwrap()
    }
}
