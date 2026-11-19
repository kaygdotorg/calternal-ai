//! Fixture-only effects over issued, revision-bound scale rows.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::authority) enum Effect {
    Pin,
    Rename,
    MoveCollection,
    Trash,
    BulkTrash,
    Restore,
    Create,
    Archive,
    MoveSection,
    CreateSection,
    RenameSection,
    RemoveSection,
    Up,
    Down,
}

impl Effect {
    pub(in crate::authority) fn challenge(self) -> bool {
        matches!(
            self,
            Self::Rename
                | Self::MoveCollection
                | Self::BulkTrash
                | Self::MoveSection
                | Self::RemoveSection
                | Self::Archive
        )
    }

    pub(in crate::authority) fn choice(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::MoveCollection => Some(("collection", "collections")),
            Self::MoveSection => Some(("section", "layout")),
            _ => None,
        }
    }
}

pub(in crate::authority) fn view_actions(app: App) -> &'static [Action] {
    use Effect::*;
    match app {
        App::Notes => &[
            Action::Display("create", Create),
            Action::Display("trash-selected", BulkTrash),
        ],
        App::Threads => &[
            Action::Display("create", Create),
            Action::Display("new-section", CreateSection),
        ],
        App::Reading => &[Action::Add],
    }
}

pub(in crate::authority) fn row_actions(app: App, bind: &str) -> &'static [Action] {
    use Effect::*;
    match (app, bind) {
        (App::Notes, "notes") => &[
            Action::Display("pin", Pin),
            Action::Display("rename", Rename),
            Action::Display("move", MoveCollection),
            Action::Display("trash", Trash),
        ],
        (App::Notes, "trash") => &[Action::Display("restore", Restore)],
        (App::Threads, "waiting") => &[
            Action::Display("move-to-waiting", MoveSection),
            Action::Display("archive-waiting", Archive),
        ],
        (App::Threads, "all") => &[
            Action::Display("move-to-all", MoveSection),
            Action::Display("archive-all", Archive),
        ],
        (App::Threads, "children") => &[
            Action::Display("move-to-children", MoveSection),
            Action::Display("archive-children", Archive),
        ],
        (App::Threads, "layout") => &[
            Action::Display("rename-section", RenameSection),
            Action::Display("remove-section", RemoveSection),
        ],
        (App::Threads, "in-section") => &[
            Action::Display("move-up-in-section", Up),
            Action::Display("move-down-in-section", Down),
            Action::Display("move-to-in-section", MoveSection),
            Action::Display("archive-in-section", Archive),
        ],
        _ => &[],
    }
}

pub(super) struct Receipt {
    outcome: Value,
    undo: Option<(Uuid, Uuid, &'static str, Value, Instant)>,
    undone: bool,
}

impl Rows {
    pub(super) fn row(&self, id: Uuid) -> Option<&Row> {
        self.active
            .iter()
            .chain(&self.trash)
            .chain(self.children.values().flatten())
            .chain(self.layouts.values().flatten())
            .chain(self.section_members.values().flatten())
            .find(|row| row.id == id)
    }

    fn row_mut(&mut self, id: Uuid) -> Option<&mut Row> {
        self.active
            .iter_mut()
            .chain(&mut self.trash)
            .chain(self.children.values_mut().flatten())
            .chain(self.layouts.values_mut().flatten())
            .chain(self.section_members.values_mut().flatten())
            .find(|row| row.id == id)
    }

    pub(super) fn revision_for(&self, row: &Row) -> String {
        let other = ref_at(&row.fields, "__thread")
            .ok()
            .and_then(|id| self.row(id))
            .unwrap_or(row);
        row_context_rev(row, other)
    }

    pub(super) fn underlying(&self, id: Uuid) -> Uuid {
        self.row(id)
            .and_then(|row| ref_at(&row.fields, "__thread").ok())
            .unwrap_or(id)
    }
}

impl Display {
    pub(super) fn placement(thread: &Row) -> Row {
        let mut row = Row::new(thread.fields.clone());
        row.fields["__thread"] = json!(thread.id);
        row
    }

    pub(in crate::authority) fn revision(&self, id: Uuid) -> Reply<String> {
        if let Some(row) = self.collections.iter().find(|row| row.id == id) {
            return Ok(row.rev.to_string());
        }
        let rows = self.rows.lock().unwrap();
        let row = rows.row(id).ok_or(HIDDEN)?;
        // A placement's context also depends on its underlying thread.
        let thread = rows.underlying(id);
        let other = rows.row(thread).ok_or(HIDDEN)?;
        if thread != id && other.fields["archived"] == true {
            return Err(STALE);
        }
        Ok(row_context_rev(row, other))
    }

    pub(in crate::authority) fn title(&self, id: Uuid) -> Reply<String> {
        let rows = self.rows.lock().unwrap();
        let row = rows.row(id).ok_or(HIDDEN)?;
        Ok(row.fields["title"].as_str().ok_or(HIDDEN)?.to_owned())
    }

    pub(in crate::authority) fn options(
        &self,
        who: AccountId,
        bind: &str,
    ) -> Vec<(Uuid, String, String)> {
        let state = self.rows.lock().unwrap();
        let choices = if bind == "layout" {
            state
                .layouts
                .get(&who)
                .map(Vec::as_slice)
                .unwrap_or_default()
        } else {
            &self.collections
        };
        choices
            .iter()
            .take(50)
            .map(|row| {
                (
                    row.id,
                    row.fields["title"].as_str().unwrap().into(),
                    row.rev.to_string(),
                )
            })
            .collect()
    }

    pub(in crate::authority) fn destination(&self, who: AccountId, id: Uuid) -> Reply<String> {
        for bind in ["collections", "layout"] {
            if let Some((_, _, version)) = self
                .options(who, bind)
                .into_iter()
                .find(|(row, _, _)| *row == id)
            {
                return Ok(version);
            }
        }
        Err(HIDDEN)
    }

    pub(in crate::authority) fn apply(
        &self,
        who: AccountId,
        id: Uuid,
        effect: Effect,
        r: &Resolution,
        token: &str,
    ) -> Reply<Value> {
        use Effect::*;
        let mut receipts = self.effects.lock().unwrap();
        receipts.retain(|_, r| r.undo.as_ref().is_none_or(|undo| undo.4 > Instant::now()));
        if let Some(receipt) = receipts.get(&(who, id)) {
            return Ok(receipt.outcome.clone());
        }
        if receipts.len() >= 4096 {
            return Err(ResultRow(StatusCode::TOO_MANY_REQUESTS, "fixture_full"));
        }
        if let Some((dest, rev)) = &r.destination {
            if self.destination(who, *dest)? != *rev {
                return Err(STALE);
            }
        }
        let mut rows = self.rows.lock().unwrap();
        for (target, rev) in &r.targets {
            let row = rows.row(*target).ok_or(HIDDEN)?;
            let other = rows.row(rows.underlying(*target)).ok_or(HIDDEN)?;
            if row_context_rev(row, other) != *rev {
                return Err(STALE);
            }
        }
        let target = r.targets.first().map(|(id, _)| *id);
        let mut outcome = json!({"activation_id":id,"state":"ok"});
        let mut undo = None;
        match effect {
            Pin | Archive => {
                let field = if effect == Pin { "pinned" } else { "archived" };
                let row_id = rows.underlying(target.ok_or(HIDDEN)?);
                let row = rows.row_mut(row_id).ok_or(HIDDEN)?;
                let before = row.fields[field].clone();
                row.fields[field] = json!(true);
                row.rev = Uuid::new_v4();
                undo = Some((
                    row_id,
                    row.rev,
                    field,
                    before,
                    Instant::now() + Duration::from_secs(600),
                ));
                outcome = super::super::undo::pin_outcome(id, token);
                if effect == Archive {
                    outcome["undo"]["label"] = json!("Undo archive");
                }
            }
            Rename | RenameSection => {
                let row = rows.row_mut(target.ok_or(HIDDEN)?).ok_or(HIDDEN)?;
                row.fields["title"] = r.material[0]["value"].clone();
                row.rev = Uuid::new_v4();
            }
            MoveCollection => {
                let dest = r.destination.as_ref().ok_or(HIDDEN)?.0;
                let index = self
                    .collections
                    .iter()
                    .position(|row| row.id == dest)
                    .ok_or(HIDDEN)?;
                let item = target.ok_or(HIDDEN)?;
                rows.collection_member.insert(item, index);
                rows.row_mut(item).ok_or(HIDDEN)?.rev = Uuid::new_v4();
            }
            Trash | BulkTrash | Restore => {
                let Rows { active, trash, .. } = &mut *rows;
                let (from, to) = if effect == Restore {
                    (trash, active)
                } else {
                    (active, trash)
                };
                if !r
                    .targets
                    .iter()
                    .all(|(id, _)| from.iter().any(|row| row.id == *id))
                {
                    return Err(STALE);
                }
                for (id, _) in &r.targets {
                    let index = from.iter().position(|row| row.id == *id).unwrap();
                    let mut row = from.remove(index);
                    row.rev = Uuid::new_v4();
                    row.fields["updated"] = json!(calternal_content::now_rfc3339());
                    to.insert(0, row);
                }
            }
            Create => {
                if rows.active.len() >= 6000 {
                    return Err(CONFLICT);
                }
                let row =
                    Self::sample(self.app, rows.active.len(), time::OffsetDateTime::now_utc());
                outcome["created"] = json!(row.id);
                rows.active.insert(0, row);
            }
            CreateSection => {
                let sections = rows.layouts.get_mut(&who).ok_or(HIDDEN)?;
                if sections.len() >= 40 {
                    return Err(CONFLICT);
                }
                let row =
                    Row::new(json!({"title":r.material[0]["value"],"position":sections.len()}));
                sections.push(row);
            }
            RemoveSection => {
                let id = target.ok_or(HIDDEN)?;
                rows.layouts
                    .get_mut(&who)
                    .ok_or(HIDDEN)?
                    .retain(|row| row.id != id);
                rows.section_members.remove(&id);
            }
            MoveSection => {
                let id = target.ok_or(HIDDEN)?;
                let dest = r.destination.as_ref().ok_or(HIDDEN)?.0;
                let thread = rows.underlying(id);
                let thread = rows.row(thread).ok_or(HIDDEN)?;
                let mut placement = Self::placement(thread);
                for list in rows.section_members.values_mut() {
                    if let Some(index) = list.iter().position(|row| row.id == id) {
                        placement = list.remove(index);
                        placement.rev = Uuid::new_v4();
                        break;
                    }
                }
                rows.section_members
                    .entry(dest)
                    .or_default()
                    .push(placement);
            }
            Up | Down => {
                let id = target.ok_or(HIDDEN)?;
                let (items, index) = rows
                    .section_members
                    .values_mut()
                    .find_map(|list| {
                        let index = list.iter().position(|row| row.id == id)?;
                        Some((list, index))
                    })
                    .ok_or(HIDDEN)?;
                let next = if effect == Up {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(items.len() - 1)
                };
                items[index].rev = Uuid::new_v4();
                items.swap(index, next);
            }
        }
        rows.generation += 1;
        receipts.insert(
            (who, id),
            Receipt {
                outcome: outcome.clone(),
                undo,
                undone: false,
            },
        );
        Ok(outcome)
    }

    pub(in crate::authority) fn undo(&self, who: AccountId, activation: Uuid) -> Reply<Value> {
        let mut receipts = self.effects.lock().unwrap();
        let receipt = receipts.get_mut(&(who, activation)).ok_or(HIDDEN)?;
        if receipt.undone {
            return Ok(json!({"activation_id":activation,"state":"ok"}));
        }
        let (id, revision, key, value, deadline) = receipt.undo.as_ref().ok_or(HIDDEN)?;
        if *deadline <= Instant::now() {
            return Err(STALE);
        }
        let mut rows = self.rows.lock().unwrap();
        let row = rows.row_mut(*id).ok_or(HIDDEN)?;
        if row.rev != *revision {
            return Err(STALE);
        }
        row.fields[*key] = value.clone();
        row.rev = Uuid::new_v4();
        rows.generation += 1;
        receipt.undone = true;
        Ok(json!({"activation_id":activation,"state":"ok"}))
    }
}

pub(super) fn row_context_rev(row: &Row, other: &Row) -> String {
    if row.id == other.id {
        row.rev.to_string()
    } else {
        format!("{}/{}", row.rev, other.rev)
    }
}

impl Host {
    pub(in crate::authority) async fn display_set(
        &self,
        who: Identity,
        session: Uuid,
        id: Uuid,
        input: &Value,
    ) -> Reply<Value> {
        let (lease, view) = self.lease(who, session, id).await?;
        let _fence = view.gate.lock().await;
        self.lease(who, session, id).await?;
        if input["bind"] != "notes" || view.display.as_ref().is_none_or(|d| d.app != App::Notes) {
            return Err(HIDDEN);
        }
        if !self.editor(who, &view).await {
            return Err(FORBIDDEN);
        }
        let mut targets = Vec::new();
        for value in input["rows"].as_array().ok_or(INVALID)? {
            let row = value
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or(HIDDEN)?;
            targets.push((row, self.display_row(lease.id, row, "notes").ok_or(HIDDEN)?));
        }
        self.check_targets(who, &view, &targets).await?;
        let mut live = self.live.lock().unwrap();
        live.display_sets.retain(|_, (id, _)| *id != lease.id);
        let token = Uuid::new_v4();
        live.display_sets.insert(token, (lease.id, targets));
        Ok(json!({"type":"selection_result","bind":"notes","selection_ref":token}))
    }

    pub(in crate::authority) fn display_row(
        &self,
        lease: Uuid,
        row: Uuid,
        bind: &str,
    ) -> Option<String> {
        self.live
            .lock()
            .unwrap()
            .windows
            .values()
            .filter(|w| w.lease == lease && w.display.as_ref().is_some_and(|(b, _)| b == bind))
            .filter_map(|w| {
                w.rows
                    .iter()
                    .find(|(id, _)| *id == row)
                    .map(|(_, rev)| (w.order, rev))
            })
            .max_by_key(|(order, _)| *order)
            .map(|(_, rev)| rev.clone())
    }
}

impl Host {
    pub(super) fn release_display_contexts(
        &self,
        live: &mut Volatile,
        lease: &Lease,
        protected: &std::collections::HashSet<Uuid>,
        needed: usize,
    ) {
        let excess = needed.saturating_sub(self.context_room(live, lease));
        if excess == 0 {
            return;
        }
        let pending: std::collections::HashSet<_> =
            live.resolutions.values().map(|r| r.context).collect();
        let evicted: Vec<_> = live
            .contexts
            .iter()
            .filter(|(id, context)| {
                context.lease == lease.id
                    && context.row.is_some_and(|row| !protected.contains(&row))
                    && !pending.contains(id)
            })
            .take(excess)
            .map(|(id, _)| *id)
            .collect();
        for id in evicted {
            live.contexts.remove(&id);
        }
    }
}
