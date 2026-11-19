//! Validation and rendering for the versioned automation Markdown document.
use super::*;
use calternal_content::{FileFormat, Item, Placement, Request, Summary};
use calternal_core::{
    AgentMode, AutomationCreate, AutomationLast, AutomationList, AutomationNext, AutomationRecord,
    AutomationResult, AutomationRevision, AutomationTarget, AutomationUpdate,
};
use calternal_types::Capability;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_DEFINITIONS: u32 = 128;

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Definition {
    pub(super) title: String,
    pub(super) creator: AccountId,
    pub(super) format: String,
    pub(super) schedule_text: String,
    pub(super) schedule: String,
    pub(super) timezone: String,
    pub(super) enabled: bool,
    pub(super) schedule_epoch: Uuid,
    pub(super) schedule_start_ms: i64,
    pub(super) clear_stop: bool,
    pub(super) mode: AgentMode,
    pub(super) machine_id: Option<String>,
    #[serde(default)]
    deleted: bool,
    pub(super) date: String,
    pub(super) updated_at: String,
}

impl Definition {
    fn read(value: Value) -> Result<Self, Error> {
        if value["format"]
            .as_str()
            .is_some_and(|format| format != FORMAT)
        {
            return Err(Error::conflict(
                "This automation uses an unsupported format. Open it with a compatible version of Calternal.",
            ).with_details(json!({"reason":"unsupported_version"})));
        }
        let definition: Self = serde_json::from_value(value)
            .map_err(|_| Error::internal("automation metadata could not be read"))?;
        Ok(definition)
    }

    fn markdown(&self, prompt: &str) -> String {
        format!(
            "---\nformat: {FORMAT}\ntitle: {}\ndate: {}\nlast edited: {}\nschedule_text: {}\nschedule: {}\ntimezone: {}\nenabled: {}\nmode: {}\n---\n\n{prompt}",
            json!(self.title),
            self.date,
            self.updated_at,
            json!(self.schedule_text),
            json!(self.schedule),
            json!(self.timezone),
            self.enabled,
            json!(self.mode),
        )
    }

    pub(super) fn from_item(item: &Item) -> Result<(Self, String), Error> {
        if item.kind != KIND || item.metadata["deleted"] == true {
            return Err(Error::not_found("automation not found"));
        }
        let definition = Self::read(item.metadata.clone())?;
        let text = std::str::from_utf8(&item.bytes)
            .map_err(|_| Error::internal("invalid automation text"))?;
        let (_, prompt) = text
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n\n"))
            .ok_or_else(|| Error::internal("invalid automation document"))?;
        Ok((definition, prompt.to_owned()))
    }

    /// Parse the saved expression or report that the schedule needs updating.
    pub(super) fn calendar(&self) -> Result<Schedule, Error> {
        Schedule::from_expression(&self.schedule).ok_or_else(|| {
            Error::conflict("This automation has an unsupported schedule. Preview and save a supported schedule.")
                .with_details(json!({"reason":"unsupported_schedule"}))
        })
    }

    fn record(
        &self,
        status: scheduling::Status,
        summary: &Summary,
        caller: AccountId,
        zone: &str,
        prompt: Option<String>,
    ) -> Result<AutomationRecord, Error> {
        let schedule = self.calendar()?;
        let is_creator = self.creator == caller;
        Ok(AutomationRecord {
            id: summary.id.to_string(),
            name: self.title.clone(),
            revision: summary.revision.clone(),
            path: summary.path.clone(),
            timing: AutomationSchedule {
                schedule_text: self.schedule_text.clone(),
                schedule: self.schedule.clone(),
                human: schedule.describe(),
                timezone: self.timezone.clone(),
                next_run_ms: if self.enabled && status.stop_reason.is_none() {
                    status
                        .next_run_ms
                        .or_else(|| schedule.next(&self.timezone, now_ms()))
                } else {
                    None
                },
            },
            enabled: self.enabled && status.stop_reason.is_none(),
            mode: self.mode,
            last: status.last,
            stop_reason: status.stop_reason,
            account_timezone: (is_creator && self.timezone != zone).then(|| zone.into()),
            machine_id: self.machine_id.clone().filter(|_| is_creator),
            prompt,
        })
    }
}

fn confirm(words: String, expression: String, timezone: String, zone: &str) -> Result<(), Error> {
    if timezone != zone || Schedule::parse(&words)?.expression() != expression {
        return Err(Error::conflict(
            "preview and confirm the schedule in your current account time zone",
        ));
    }
    Ok(())
}

fn name(name: String) -> Result<String, Error> {
    let name = name.trim().to_owned();
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err(Error::invalid_input(
            "give the automation a name of up to 256 UTF-8 bytes",
        ));
    }
    Ok(name)
}

fn prompt(value: String) -> Result<String, Error> {
    if value.trim().is_empty() || value.len() > 16 * 1024 {
        return Err(Error::invalid_input(
            "the prompt must contain 1 to 16384 UTF-8 bytes",
        ));
    }
    Ok(value)
}

fn machine(value: Option<String>) -> Result<Option<String>, Error> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| {
            Uuid::parse_str(&value)
                .map(|id| id.to_string())
                .map_err(|_| Error::invalid_input("machine_id must be a UUID"))
        })
        .transpose()
}

fn summarize(store: &crate::store::Store, item: &Item) -> Result<Summary, Error> {
    let path = store.content().file_path(item.space, &item.kind, item.id)?;
    let relative = path
        .parent()
        .and_then(|p| p.file_name())
        .zip(path.file_name())
        .ok_or_else(|| Error::internal("invalid saved automation path"))?;
    Ok(Summary {
        id: item.id,
        space: item.space,
        kind: item.kind.clone(),
        path: format!(
            "AI/{}/{}",
            relative.0.to_string_lossy(),
            relative.1.to_string_lossy()
        ),
        metadata: item.metadata.clone(),
        revision: item.revision.clone(),
        updated_at: item.updated_at.clone(),
    })
}

pub(super) fn dispatch<D: Directory>(
    state: &Shared<D>,
    who: Identity,
    actor: Actor,
    space: SpaceId,
    rid: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let zone = account_zone(state, who.id)?;
    let content = state.store.content();
    if op == "automations.list" {
        let rows = content.list(space, KIND, MAX_DEFINITIONS)?;
        let mut automations = Vec::with_capacity(rows.len());
        let mut statuses =
            scheduling::statuses(&state.store, space, rows.iter().map(|row| row.id))?;
        for row in rows {
            let definition = Definition::read(row.metadata.clone())?;
            let status = statuses.remove(&row.id.to_string()).unwrap_or_default();
            automations.push(definition.record(status, &row, who.id, &zone, None)?);
        }
        return Ok(json!(AutomationList { automations }));
    }

    let id = input
        .get("id")
        .and_then(Value::as_str)
        .map(crate::store::item_id)
        .transpose()?;
    if matches!(
        op,
        "automations.read" | "automations.next_run" | "automations.last_run"
    ) {
        let _: AutomationTarget = decode(input)?;
        let item = content.get(
            space,
            id.ok_or_else(|| Error::not_found("automation not found"))?,
        )?;
        let (definition, prompt) = Definition::from_item(&item)?;
        let record = definition.record(
            scheduling::status(&state.store, space, item.id)?,
            &summarize(&state.store, &item)?,
            who.id,
            &zone,
            (op == "automations.read").then_some(prompt),
        )?;
        return Ok(match op {
            "automations.next_run" => json!(AutomationNext {
                next_run_ms: record.timing.next_run_ms
            }),
            "automations.last_run" => json!(AutomationLast { last: record.last }),
            _ => json!(AutomationResult { automation: record }),
        });
    }
    let fp = crate::api::fingerprint(op, &input);
    let request = Request {
        account: who.id,
        via: actor.via.as_ref(),
        id: rid,
        operation: op,
        fingerprint: &fp,
    };
    if let Some(receipt) = content.receipt(space, who.id, rid)? {
        if receipt.operation != op || receipt.fingerprint != fp {
            return Err(Error::request_reused());
        }
        return complete(state, who.id, &zone, space, op, receipt.result);
    }
    let perms = state
        .directory
        .capabilities(who.id, space)
        .ok_or_else(|| Error::not_found("space not found"))?;
    let stamp = crate::store::now_rfc3339();
    let (definition, body, item, base) = if op == "automations.create" {
        if !perms.contains(Capability::PluginsUse) {
            return Err(Error::forbidden("plugin use is required for automations"));
        }
        let i: AutomationCreate = decode(input)?;
        confirm(
            i.schedule_text.clone(),
            i.schedule.clone(),
            i.timezone.clone(),
            &zone,
        )?;
        scheduling::watch(&state.store, space)?;
        content.register_kind(
            space,
            KIND,
            Placement::VisibleTree {
                folder: "AI".into(),
                subdirectories: vec!["Automations".into()],
            },
            FileFormat::Markdown,
        )?;
        (
            Definition {
                title: name(i.name)?,
                creator: who.id,
                format: FORMAT.into(),
                schedule_text: i.schedule_text,
                schedule: i.schedule,
                timezone: i.timezone,
                enabled: true,
                schedule_epoch: Uuid::now_v7(),
                schedule_start_ms: now_ms(),
                clear_stop: true,
                mode: i.mode.unwrap_or_default(),
                machine_id: machine(i.machine_id)?,
                deleted: false,
                date: stamp.clone(),
                updated_at: stamp,
            },
            prompt(i.prompt)?,
            None,
            None,
        )
    } else {
        let item = content.get(
            space,
            id.ok_or_else(|| Error::not_found("automation not found"))?,
        )?;
        let (mut definition, mut body) = Definition::from_item(&item)?;
        let wants_execution = matches!(op, "automations.update" | "automations.enable");
        if definition.creator != who.id
            && (wants_execution || !perms.contains(Capability::SpaceSettings))
        {
            return Err(Error::forbidden(
                "only the creator can configure or start this automation",
            ));
        }
        if wants_execution && !perms.contains(Capability::PluginsUse) {
            return Err(Error::forbidden("plugin use is required for automations"));
        }
        definition.updated_at = stamp;
        definition.clear_stop = op == "automations.enable";
        let (base, reschedule) = if op == "automations.update" {
            let i: AutomationUpdate = decode(input)?;
            if let Some(value) = i.name {
                definition.title = name(value)?;
            }
            if let Some(value) = i.prompt {
                body = prompt(value)?;
            }
            if let Some(mode) = i.mode {
                definition.mode = mode;
            }
            if let Some(value) = i.machine_id {
                definition.machine_id = machine(Some(value))?;
            }
            let reschedule = match (i.schedule_text, i.schedule, i.timezone) {
                (Some(words), Some(expression), Some(timezone)) => {
                    confirm(words.clone(), expression.clone(), timezone.clone(), &zone)?;
                    definition.schedule_text = words;
                    definition.schedule = expression;
                    definition.timezone = timezone;
                    true
                }
                (None, None, None) => false,
                _ => {
                    return Err(Error::invalid_input(
                        "send the three confirmed schedule fields together",
                    ));
                }
            };
            (i.base_revision, reschedule)
        } else {
            let i: AutomationRevision = decode(input)?;
            definition.enabled = op == "automations.enable";
            (i.base_revision, true)
        };
        if reschedule {
            definition.schedule_epoch = Uuid::now_v7();
            definition.schedule_start_ms = now_ms();
        }
        (definition, body, Some(item), Some(base))
    };
    if let Some(machine) = definition.machine_id.as_deref() {
        if matches!(
            op,
            "automations.create" | "automations.update" | "automations.enable"
        ) {
            scheduling::validate_machine(state, definition.creator, machine)?;
        }
    }

    let result = |item: &Item| json!({"id":item.id});
    let bytes = definition.markdown(&body);
    let mut metadata = json!(definition);
    let saved = if op == "automations.remove" {
        metadata["deleted"] = json!(true);
        let item = item.ok_or_else(|| Error::not_found("automation not found"))?;
        if base.as_deref() != Some(&item.revision) {
            return Err(Error::conflict("item has a newer revision"));
        }
        content.trash(
            &item,
            metadata,
            bytes.as_bytes(),
            request,
            result,
            || Ok(()),
        )?
    } else if op == "automations.create" {
        content.create_bounded(
            space,
            KIND,
            MAX_DEFINITIONS,
            metadata,
            bytes.as_bytes(),
            request,
            result,
        )?
    } else {
        content.write(
            space,
            item.as_ref().map(|item| item.id),
            KIND,
            base.as_deref(),
            metadata,
            bytes.as_bytes(),
            request,
            result,
        )?
    };
    complete(state, who.id, &zone, space, op, result(&saved))
}

fn complete<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    zone: &str,
    space: SpaceId,
    op: &str,
    result: Value,
) -> Result<Value, Error> {
    let id = result["id"]
        .as_str()
        .map(crate::store::item_id)
        .transpose()?
        .ok_or_else(|| Error::internal("missing saved automation"))?;
    if op == "automations.remove" {
        scheduling::remove(&state.store, space, id)?;
        return Ok(json!({"ok":true}));
    }
    state.store.content().materialize(space, id)?;
    let item = state.store.content().get(space, id)?;
    let (definition, prompt) = Definition::from_item(&item)?;
    scheduling::register(&state.store, &item, &definition)?;
    Ok(json!(AutomationResult {
        automation: definition.record(
            scheduling::status(&state.store, space, item.id)?,
            &summarize(&state.store, &item)?,
            account,
            zone,
            Some(prompt),
        )?
    }))
}
