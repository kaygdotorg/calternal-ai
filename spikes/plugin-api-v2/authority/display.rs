//! In-memory renderer data sets. Storage and authority timings use the original views.
use super::*;
#[path = "display_actions.rs"]
pub(super) mod actions;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum App {
    Notes,
    Threads,
    Reading,
}
impl App {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Notes => "notes",
            Self::Threads => "threads",
            Self::Reading => "reading",
        }
    }

    pub(super) fn template(self) -> Value {
        let src = match self {
            Self::Notes => include_str!("rev13/notes-app.json"),
            Self::Threads => include_str!("rev13/threads-nested-catalogue-app.json"),
            Self::Reading => {
                include_str!("rev13/reading-list-app.json")
            }
        };
        let mut template: Value = serde_json::from_str(src).unwrap();
        if self == Self::Notes {
            let bulk: Value =
                serde_json::from_str(include_str!("rev13/notes-bulk-and-snooze-app.json")).unwrap();
            template["chrome"]["header"]["trailing"]
                .as_array_mut()
                .unwrap()
                .push(bulk["view"]["root"]["children"][1]["children"][0]["invoke"].clone());
        }
        template
    }

    pub(super) fn manifest(self) -> Value {
        let src = match self {
            Self::Notes => {
                include_str!("rev13/manifests/notes-app.json")
            }
            Self::Threads => include_str!("rev13/manifests/threads-nested-catalogue-app.json"),
            Self::Reading => include_str!("rev13/manifests/reading-list-app.json"),
        };
        let mut manifest: Value = serde_json::from_str(src).unwrap();
        if self == Self::Notes {
            let bytes = serde_json::to_vec(&self.template()).unwrap();
            manifest["apps"][0]["description"]["template"]["sha256"] = json!(hex::encode(
                ring::digest::digest(&ring::digest::SHA256, &bytes)
            ));
        }
        manifest
    }
}

#[derive(Clone)]
struct Row {
    id: Uuid,
    rev: Uuid,
    fields: Value,
}
impl Row {
    fn new(fields: Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            rev: Uuid::new_v4(),
            fields,
        }
    }
    fn wire(&self, bind: &str, actor: AccountId) -> Value {
        let fields = &self.fields;
        let fields = match bind {
            "current" => json!({"title":fields["title"]}),
            "note" => {
                json!({"title":fields["title"],"updated":fields["updated"],"pinned":fields["pinned"]})
            }
            "note-search" => {
                json!({"title":fields["title"],"updated":fields["updated"],"snippet":fields["snippet"]})
            }
            "trash-count" => json!({}),
            "trash" => {
                json!({"title":fields["title"],"trashed_at":fields["updated"],"updated":fields["updated"],"snippet":fields["snippet"]})
            }
            "waiting" => {
                json!({"title":fields["title"],"state":"waiting","waiting_on":actor})
            }
            "all" => {
                json!({"title":fields["title"],"state":fields["state"],"updated":fields["updated"]})
            }
            "children" | "in-section" => {
                json!({"title":fields["title"],"state":fields["state"]})
            }
            "reading-search" => json!({"title":fields["title"],"read":fields["read"]}),
            "items" => {
                json!({"title":fields["title"],"read":fields["read"],"added":fields["added"]})
            }
            _ => fields.clone(),
        };
        json!({"id":self.id,"fields":fields})
    }
}

struct Rows {
    active: Vec<Row>,
    trash: Vec<Row>,
    generation: u64,
    change: u64,
    ephemeral: Option<Uuid>,
    children: HashMap<Uuid, Vec<Row>>,
    layouts: HashMap<AccountId, Vec<Row>>,
    collection_member: HashMap<Uuid, usize>,
    section_members: HashMap<Uuid, Vec<Row>>,
}
pub(super) struct Display {
    pub(super) app: App,
    rows: Mutex<Rows>,
    collections: Vec<Row>,
    reading_receipts: Mutex<HashMap<(AccountId, Uuid), Value>>,
    effects: Mutex<HashMap<(AccountId, Uuid), actions::Receipt>>,
}

impl Display {
    pub(super) fn new(app: App, size: usize, actors: [AccountId; 2]) -> Arc<Self> {
        let now = time::OffsetDateTime::now_utc();
        let active: Vec<Row> = (0..size)
            .map(|i| Self::sample(app, i, now - time::Duration::seconds(i as i64)))
            .collect();
        let collections = if app == App::Notes {
            ["Projects", "Reference"]
                .into_iter()
                .map(|name| Row::new(json!({"title":name})))
                .collect()
        } else {
            vec![]
        };
        let collection_member = active
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id, i % 2))
            .collect();
        let children = if app == App::Threads {
            active
                .iter()
                .take(32)
                .enumerate()
                .map(|(i, parent)| {
                    let items = (0..6)
                        .map(|j| {
                            let mut row =
                                Self::sample(app, j, now - time::Duration::seconds(j as i64));
                            row.fields["title"] =
                                json!(format!("Thread {:05} · Child {}", i + 1, j + 1));
                            row
                        })
                        .collect();
                    (parent.id, items)
                })
                .collect()
        } else {
            HashMap::new()
        };
        let layouts: HashMap<_, Vec<_>> = if app == App::Threads {
            actors
                .into_iter()
                .map(|actor| {
                    (
                        actor,
                        ["In progress", "Reference"]
                            .into_iter()
                            .enumerate()
                            .map(|(i, title)| Row::new(json!({"title":title,"position":i})))
                            .collect(),
                    )
                })
                .collect()
        } else {
            HashMap::new()
        };
        let mut section_members = HashMap::new();
        for sections in layouts.values() {
            for (section, item) in sections.iter().enumerate() {
                let placements = active
                    .iter()
                    .skip(section * 6)
                    .take(6)
                    .map(|thread| Self::placement(thread))
                    .collect();
                section_members.insert(item.id, placements);
            }
        }
        let trash = if app == App::Notes {
            (0..3).map(|i| Self::sample(app, size + i, now)).collect()
        } else {
            vec![]
        };
        Arc::new(Self {
            app,
            rows: Mutex::new(Rows {
                active,
                trash,
                generation: 0,
                change: 0,
                ephemeral: None,
                children,
                layouts,
                collection_member,
                section_members,
            }),
            collections,
            reading_receipts: Mutex::default(),
            effects: Mutex::default(),
        })
    }

    fn sample(app: App, i: usize, at: time::OffsetDateTime) -> Row {
        let at = calternal_content::format_rfc3339(at);
        Row::new(match app {
            App::Notes => json!({"title":format!("Note {:05}",i+1),"updated":at,"pinned":i%17==0,
                "snippet":format!("Fixture note {} · A short excerpt for the leading preview and second line.",i+1)}),
            App::Threads => json!({"title":format!("Thread {:05}",i+1),"updated":at,
                "state":"idle"}),
            App::Reading => json!({"title":format!("Reading item {}",i+1),"added":at,"read":false}),
        })
    }

    pub(super) fn count(&self) -> usize {
        self.rows.lock().unwrap().active.len()
    }

    pub(super) fn generation(&self) -> u64 {
        self.rows.lock().unwrap().generation
    }

    pub(super) fn churn(&self) {
        let mut rows = self.rows.lock().unwrap();
        let now = time::OffsetDateTime::now_utc();
        let serial = rows.change;
        rows.change += 1;
        rows.generation += 1;
        match serial % 3 {
            0 => {
                let row = Self::sample(self.app, 10_000 + serial as usize, now);
                rows.ephemeral = Some(row.id);
                rows.active.insert(0, row);
            }
            1 => {
                let Some(index) = rows.active.len().min(4).checked_sub(1) else {
                    return;
                };
                let mut row = rows.active.remove(index);
                row.rev = Uuid::new_v4();
                row.fields["title"] = json!(format!("Renamed fixture {}", serial));
                row.fields["updated"] = json!(calternal_content::format_rfc3339(now));
                rows.active.insert(0, row);
            }
            _ => {
                // Trash the churn-created row, preserving all seeded IDs and parent edges.
                let ephemeral = rows.ephemeral.take();
                let index = rows.active.iter().position(|row| Some(row.id) == ephemeral);
                if let Some(index) = index {
                    let mut row = rows.active.remove(index);
                    row.rev = Uuid::new_v4();
                    row.fields["updated"] = json!(calternal_content::format_rfc3339(now));
                    if self.app == App::Notes {
                        rows.trash.insert(0, row);
                        rows.trash.truncate(200);
                    }
                }
            }
        }
    }

    pub(super) fn add(&self, actor: AccountId, activation: Uuid, input: &Value) -> Reply<Value> {
        let (title, url) = reading_input(input)?;
        let mut receipts = self.reading_receipts.lock().unwrap();
        if let Some(prior) = receipts.get(&(actor, activation)) {
            if prior["input"] != *input {
                return Err(CONFLICT);
            }
            return Ok(prior["result"].clone());
        }
        let mut rows = self.rows.lock().unwrap();
        if rows.active.len() >= 1000 {
            return Err(ResultRow(StatusCode::TOO_MANY_REQUESTS, "fixture_full"));
        }
        let row = Row::new(
            json!({"title":title,"url":url,"added":calternal_content::now_rfc3339(),"read":false}),
        );
        let result = json!({"activation_id":activation,"state":"ok","created":row.id});
        rows.active.insert(0, row);
        rows.generation += 1;
        receipts.insert((actor, activation), json!({"input":input,"result":result}));
        Ok(result)
    }
}

pub(super) fn reading_input(input: &Value) -> Reply<(&str, &str)> {
    if input.as_object().is_none_or(|o| o.len() != 1)
        || input["form"].as_object().is_none_or(|o| o.len() != 2)
    {
        return Err(INVALID);
    }
    let title = input["form"]["title"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.chars().count() <= 120)
        .ok_or(INVALID)?;
    let url = input["form"]["url"]
        .as_str()
        .filter(|s| s.len() <= 500)
        .ok_or(INVALID)?;
    let valid = url::Url::parse(url).is_ok_and(|u| {
        matches!(u.scheme(), "https" | "http")
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none()
    });
    if !valid {
        return Err(INVALID);
    }
    Ok((title, url))
}

#[derive(Clone, Default)]
pub(super) struct Route {
    pub(super) item: Option<Uuid>,
    pub(super) collection: Option<Uuid>,
}

impl Host {
    pub(crate) fn start_churn(self: &Arc<Self>, rate: u64) -> Option<tokio::task::JoinHandle<()>> {
        if rate == 0 {
            return None;
        }
        let displays = self
            .views
            .values()
            .filter_map(|view| view.display.as_ref())
            .filter(|display| display.app != App::Reading)
            .cloned()
            .collect::<Vec<_>>();
        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs_f64(1.0 / rate as f64));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            interval.tick().await;
            loop {
                interval.tick().await;
                for display in &displays {
                    display.churn();
                }
            }
        }))
    }

    pub(super) async fn display_search(
        &self,
        actor: Identity,
        session: Uuid,
        id: Uuid,
        input: &Value,
    ) -> Reply<Value> {
        let (lease, view) = self.lease(actor, session, id).await?;
        let display = view.display.ok_or(HIDDEN)?;
        let search_bind = match display.app {
            App::Notes => "note-search",
            App::Reading => "reading-search",
            App::Threads => return Err(HIDDEN),
        };
        let text = input["text"]
            .as_str()
            .filter(|s| s.chars().count() <= 200)
            .ok_or(INVALID)?;
        if input["type"] != "search"
            || input.as_object().is_none_or(|o| {
                o.keys()
                    .any(|k| !["type", "text", "bind", "match"].contains(&k.as_str()))
            })
            || input
                .get("match")
                .is_some_and(|m| m != "terms" && m != "prefix")
            || (input.get("bind").is_some() && input["bind"] != search_bind)
        {
            return Err(INVALID);
        }
        self.live
            .lock()
            .unwrap()
            .display_search
            .insert(lease.id, (text.to_lowercase(), input["match"] == "prefix"));
        Ok(json!({"bind":search_bind}))
    }

    pub(super) async fn display_window(
        &self,
        actor: Identity,
        lease: &Lease,
        view: &View,
        display: &Display,
        input: &Value,
        route: &Route,
    ) -> Reply<Value> {
        let _fence = view.gate.lock().await;
        self.lease(actor, lease.session, lease.id).await?;
        let person = actor.id;
        let bind = input["bind"].as_str().ok_or(INVALID)?;
        let from = input["from"].as_u64().ok_or(INVALID)?;
        let to = input["to"].as_u64().ok_or(INVALID)?;
        let end = input["anchor"] == "end";
        if from >= to || to - from > 200 || (!end && from > 500_000) {
            return Err(INVALID);
        }
        let parent = input
            .get("parent")
            .map(|_| ref_at(input, "parent"))
            .transpose()?;
        let editor = self.editor(actor, view).await;
        let mut live = self.live.lock().unwrap();
        let parent_row = parent
            .map(|id| {
                live.display_parents
                    .get(&id)
                    .filter(|p| p.0 == lease.id && p.1 == bind)
                    .map(|p| p.2)
                    .ok_or(HIDDEN)
            })
            .transpose()?;
        if matches!(bind, "children" | "in-section") != parent.is_some() {
            return Err(HIDDEN);
        }
        let rows = display.rows.lock().unwrap();
        let matches_search = |text: &str| {
            let text = text.to_lowercase();
            live.display_search
                .get(&lease.id)
                .is_none_or(|(query, prefix)| {
                    if *prefix {
                        text.split_whitespace().any(|term| term.starts_with(query))
                    } else {
                        query.split_whitespace().all(|term| text.contains(term))
                    }
                })
        };
        let search_page = matches!(bind, "note-search" | "reading-search");
        let has_count = !search_page && !matches!(bind, "note" | "current");
        if end && !has_count {
            return Ok(
                json!({"status":422,"code":"unsupported_query_plan","reason":"end_unavailable"}),
            );
        }
        let member = route
            .collection
            .map(|id| {
                display
                    .collections
                    .iter()
                    .position(|r| r.id == id)
                    .ok_or(HIDDEN)
            })
            .transpose()?;
        let mut filtered = Vec::new();
        let mut counted: Option<&[Row]> = None;
        match (display.app, bind) {
            (App::Notes, "notes" | "note-search") if member.is_some() || search_page => {
                filtered.extend(rows.active.iter().filter(|row| {
                    member.is_none_or(|m| rows.collection_member.get(&row.id) == Some(&m))
                        && (!search_page
                            || matches_search(&format!(
                                "{} {}",
                                row.fields["title"], row.fields["snippet"]
                            )))
                }));
            }
            (App::Notes, "notes") | (App::Reading, "items") => counted = Some(&rows.active),
            (App::Notes, "trash" | "trash-count") => counted = Some(&rows.trash),
            (App::Notes, "collections") => counted = Some(&display.collections),
            (App::Notes, "note" | "current") => filtered.extend(
                rows.active
                    .iter()
                    .filter(|r| route.item == Some(r.id))
                    .take(1),
            ),
            (App::Threads, "all") => filtered.extend(
                rows.active
                    .iter()
                    .filter(|row| row.fields["archived"] != true),
            ),
            (App::Threads, "waiting") => filtered.extend(
                rows.active
                    .iter()
                    .filter(|row| row.fields["archived"] != true)
                    .take(3),
            ),
            (App::Threads, "children") => {
                let parent = parent_row.ok_or(HIDDEN)?;
                if rows
                    .row(parent)
                    .is_none_or(|row| row.fields["archived"] == true)
                {
                    return Err(HIDDEN);
                }
                filtered.extend(
                    rows.children
                        .get(&parent)
                        .ok_or(HIDDEN)?
                        .iter()
                        .filter(|row| row.fields["archived"] != true),
                );
            }
            (App::Threads, "layout") => counted = Some(rows.layouts.get(&person).ok_or(HIDDEN)?),
            (App::Threads, "in-section") => {
                let section = parent_row.ok_or(HIDDEN)?;
                if !rows.layouts[&person].iter().any(|row| row.id == section) {
                    return Err(HIDDEN);
                }
                filtered.extend(
                    rows.section_members
                        .get(&section)
                        .into_iter()
                        .flatten()
                        .filter(|row| {
                            rows.row(rows.underlying(row.id))
                                .is_some_and(|thread| thread.fields["archived"] != true)
                        }),
                );
            }
            (App::Reading, "reading-search") => filtered.extend(
                rows.active
                    .iter()
                    .filter(|row| matches_search(row.fields["title"].as_str().unwrap())),
            ),
            (App::Reading, "reminders") => {}
            _ => return Err(HIDDEN),
        }
        let total = counted.map_or(filtered.len(), <[Row]>::len) as u64;
        let (lo, hi) = if end {
            (total.saturating_sub(to), total.saturating_sub(from))
        } else {
            (from.min(total), to.min(total))
        };
        let slice = lo as usize..hi as usize;
        let selected: Vec<_> = match counted {
            Some(rows) => rows[slice].iter().collect(),
            None => filtered[slice].to_vec(),
        };
        let window_ref = Uuid::new_v4();
        let version = Uuid::new_v4();
        let mut wire_rows = Vec::with_capacity(selected.len());
        let mut contexts = Vec::new();
        let row_actions = actions::row_actions(display.app, bind);
        if editor && !row_actions.is_empty() {
            let selected_ids: std::collections::HashSet<_> =
                selected.iter().map(|row| row.id).collect();
            let retained = live
                .contexts
                .values()
                .filter(|context| {
                    context.lease == lease.id
                        && context.row.is_some_and(|id| selected_ids.contains(&id))
                        && row_actions.contains(&context.action)
                })
                .count();
            let needed = (selected.len() * row_actions.len())
                .min(256)
                .saturating_sub(retained);
            self.release_display_contexts(&mut live, lease, &selected_ids, needed);
        }
        for row in &selected {
            let mut body = row.wire(bind, person);
            let edge = match bind {
                "all" if rows.children.contains_key(&row.id) => Some("children"),
                "layout" => Some("in-section"),
                _ => None,
            };
            if let Some(edge) = edge {
                let key = (lease.id, edge, row.id);
                let token = live
                    .display_parents
                    .iter()
                    .find_map(|(id, r)| (*r == key).then_some(*id))
                    .unwrap_or_else(Uuid::new_v4);
                live.display_parents.insert(token, key);
                body["parent_refs"] = json!({(edge):token});
            }
            if editor && !actions::row_actions(display.app, bind).is_empty() {
                let issued = self.mint_in(
                    &mut live,
                    lease,
                    Some((row.id, rows.revision_for(row))),
                    actions::row_actions(display.app, bind),
                );
                if issued.as_object().is_some_and(|map| !map.is_empty()) {
                    contexts.push(json!({"row":row.id,"contexts":issued}));
                }
            }
            wire_rows.push(body);
        }
        let generation = rows.generation;
        live.window_order += 1;
        let order = live.window_order;
        live.windows.insert(
            window_ref,
            IssuedWindow {
                lease: lease.id,
                version,
                order,
                rows: selected
                    .iter()
                    .map(|row| (row.id, rows.revision_for(row)))
                    .collect(),
                display: Some((bind.into(), generation)),
            },
        );
        // Keep only the lease's most recent 64 issued windows.
        let oldest = live
            .windows
            .iter()
            .filter(|(_, w)| w.lease == lease.id)
            .count();
        if oldest > 64 {
            if let Some(id) = live
                .windows
                .iter()
                .filter(|(_, w)| w.lease == lease.id)
                .min_by_key(|(_, w)| w.order)
                .map(|(k, _)| *k)
            {
                live.windows.remove(&id);
            }
        }
        let mut reply = json!({"bind":bind,"state":if total==0 && from==0 {"empty"}else{"ready"},"offset":from,
            "rows":wire_rows,"window_ref":window_ref,"result_version":version,"audience":lease.audience,"activation_contexts":contexts});
        if end {
            reply["anchor"] = json!("end");
        }
        if let Some(parent) = parent {
            reply["parent"] = json!(parent);
        }
        if has_count {
            reply["count"] = json!({"mode":"exact","value":total});
        }
        Ok(reply)
    }

    pub(super) fn display_selection(
        &self,
        lease: &Lease,
        display: &Display,
        input: &Value,
        editor: bool,
    ) -> Reply<Value> {
        let row = ref_at(input, "row")?;
        let bind = input["bind"].as_str().ok_or(INVALID)?;
        let revision = display.revision(row)?;
        let mut live = self.live.lock().unwrap();
        if !live.windows.values().any(|w| {
            w.lease == lease.id
                && w.display.as_ref().is_some_and(|(b, _)| b == bind)
                && w.rows
                    .iter()
                    .any(|(id, rev)| *id == row && *rev == revision)
        }) {
            return Err(STALE);
        }
        let contexts = if editor {
            let requested = Some((row, revision));
            let actions = actions::row_actions(display.app, bind);
            let mut issued = self.mint_in(&mut live, lease, requested.clone(), actions);
            if !actions.is_empty() && issued.as_object().is_some_and(|map| map.is_empty()) {
                self.release_display_contexts(
                    &mut live,
                    lease,
                    &[row].into_iter().collect(),
                    actions.len(),
                );
                issued = self.mint_in(&mut live, lease, requested, actions);
            }
            issued
        } else {
            json!({})
        };
        Ok(
            json!({"type":"selection_context","lease_ref":lease.id,"cache_partition":lease.partition,
            "bind":bind,"row":row,"activation_contexts":contexts}),
        )
    }
}
