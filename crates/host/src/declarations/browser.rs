//! Browser presentation metadata projected from the installed format catalog.

use super::as_json;
use crate::space::Space;
use calternal_format::{
    self as format, Field, FieldType, FillBinding, OperationForm, PageEntry, Source,
};
use calternal_types::Actor;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub(crate) fn describe(space: &Space, actor: &Actor) -> Value {
    let mut plugins = BTreeMap::new();
    for (id, plugin) in &space.plugins {
        let kinds: Map<_, _> = plugin
            .kinds
            .iter()
            .filter_map(|(key, kind)| {
                let qualified = qualify(id, key);
                space.permits(actor, &qualified, false).ok()?;
                Some((key.clone(), kind_json(id, &qualified, kind)))
            })
            .collect();
        let collections: Map<_, _> = plugin
            .collections
            .iter()
            .filter_map(|(key, collection)| {
                let [Source::Kind(from)] = collection.sources.as_slice() else {
                    return None;
                };
                let from = qualify(id, from);
                if collection.share_source == format::ShareSource::Shared {
                    return None;
                }
                space.permits(actor, &from, false).ok()?;
                let views = collection
                    .compiled_views
                    .iter()
                    .map(|view| json!({"type":view.kind,"columns":view.columns}))
                    .collect::<Vec<_>>();
                let sort = collection
                    .sort
                    .iter()
                    .map(|sort| {
                        let direction = if sort.descending { " desc" } else { "" };
                        format!("{}{direction}", sort.field)
                    })
                    .collect::<Vec<_>>();
                Some((
                    key.clone(),
                    json!({
                        "id":key,"qualified":qualify(id,key),"plugin":id,"title":collection.title,
                        "from":from,"sort":sort,"views":views,"icon":collection.icon
                    }),
                ))
            })
            .collect();
        // Only the person's shell or the owning plugin can see plugin surface
        // details beyond visible kind schemas and collections.
        let shell = actor.via.is_none() || crate::identity(actor).0 == id;
        if !shell && kinds.is_empty() && collections.is_empty() {
            continue;
        }
        let manifest = &plugin.manifest;
        plugins.insert(id, json!({
            "id":id,"name":manifest.name,"version":manifest.version,"icon":manifest.icon.as_deref().unwrap_or_default(),
            "description":manifest.body,"kinds":kinds,"collections":collections,
            "sidebars":{},"slots":{},"fills":{}
        }));
    }
    for (id, view) in &mut plugins {
        if actor.via.is_some() && crate::identity(actor).0 != id.as_str() {
            continue;
        }
        let plugin = &space.plugins[*id];
        let sidebars: Map<_, _> = plugin
            .sidebars
            .iter()
            .map(|(key, sidebar)| {
                let entries = sidebar
                    .entries
                    .iter()
                    .filter_map(|entry| sidebar_entry(space, actor, id, entry))
                    .collect::<Vec<_>>();
                (
                    key.clone(),
                    json!({"id":key,"plugin":id,"title":sidebar.title,"entries":entries}),
                )
            })
            .collect();
        view["sidebars"] = Value::Object(sidebars);
        view["slots"] = Value::Object(plugin.manifest.slots.iter().map(|(key, slot)| {
            let cardinality = if slot.multiple { "many" } else { "single" };
            let slot_id = format!("{id}:{key}");
            let fillers = space.plugins.iter().filter_map(|(provider, bundle)| {
                let fill = bundle.manifest.fills().find(|fill| fill.place == slot_id)?;
                let (kind, target) = visible_fill(space, actor, provider, fill)?;
                let label = fill.label.as_deref().unwrap_or(&bundle.manifest.name);
                Some(json!({"plugin":provider,"kind":kind,"target":target,"label":label}))
            }).collect::<Vec<_>>();
            let composer = slot.composer.as_ref().map(|binding| json!({
                "text":binding.text,"draft":binding.draft,"quote":binding.quote
            }));
            (key.clone(), json!({
                "name":key,"cardinality":cardinality,"gives":slot.gives,"pickedBy":if slot.picked_by_space {"space"} else {"person"},
                "recommended":slot.recommended,"inputOperation":slot.input_operation,"fillers":fillers,
                "label":slot.label,"composer":composer
            }))
        }).collect());
        view["fills"] = Value::Object(
            plugin
                .manifest
                .fills()
                .filter_map(|fill| {
                    let (kind, target) = visible_fill(space, actor, id, fill)?;
                    let label = fill.label.as_deref().unwrap_or(&plugin.manifest.name);
                    Some((
                        fill.place.clone(),
                        json!({
                            "slot":fill.place,"kind":kind,"target":target,"label":label
                        }),
                    ))
                })
                .collect(),
        );
    }
    json!({"space":space.id,"plugins":plugins.into_values().collect::<Vec<_>>()})
}

fn sidebar_entry(
    space: &Space,
    actor: &Actor,
    plugin: &str,
    entry: &format::SidebarEntry,
) -> Option<Value> {
    let Some((collection, owner)) = &entry.collection_ref else {
        let title = entry.title.as_ref()?;
        return Some(json!({"type":"section","title":title}));
    };
    let owner = owner.as_deref().unwrap_or(plugin);
    let qualified = qualify(owner, collection);
    let collection = space.plugins.get(owner)?.collections.get(collection)?;
    let [Source::Kind(from)] = collection.sources.as_slice() else {
        return None;
    };
    if collection.share_source == format::ShareSource::Shared {
        return None;
    }
    space.permits(actor, &qualify(owner, from), false).ok()?;
    let title = entry.title.as_deref().unwrap_or(&collection.title);
    Some(json!({"type":"collection","title":title,"collection":qualified,"icon":entry.icon}))
}

fn visible_fill<'a>(
    space: &Space,
    actor: &Actor,
    provider: &str,
    fill: &'a format::Fill,
) -> Option<(&'static str, &'a str)> {
    if actor.via.is_some() && crate::identity(actor).0 != provider {
        return None;
    }
    let (owner, key) = fill.slot.as_ref()?;
    space.plugins.get(owner)?.manifest.slots.get(key)?;
    Some(match &fill.binding {
        FillBinding::Operation(target) => ("operation", target),
        FillBinding::Frame(target) => ("frame", target),
        FillBinding::Collection(target) => ("collection", target),
        FillBinding::Receive { .. } => return None,
    })
}

fn qualify(plugin: &str, key: &str) -> String {
    if key.contains('.') {
        key.to_owned()
    } else {
        format!("{plugin}.{key}")
    }
}

fn kind_json(plugin: &str, qualified: &str, kind: &format::Kind) -> Value {
    let page = kind
        .compiled_page
        .iter()
        .map(|entry| match entry {
            PageEntry::Body => json!({"kind":"body"}),
            PageEntry::Properties => json!({"kind":"properties"}),
            PageEntry::Records(None) => json!({"kind":"records"}),
            PageEntry::Records(Some(fields)) => json!({"kind":"records","recordFields":fields}),
            PageEntry::Field(name) => json!({"kind":"field","name":name}),
        })
        .collect::<Vec<_>>();
    json!({
        "id":kind.id,"qualified":qualified,"plugin":plugin,"name":kind.name,"folder":kind.folder,
        "editable":if kind.editable == format::Editability::Outside {"outside"} else {"operations"},
        "fields":fields_json(&kind.fields),"records":fields_json(&kind.records),"state":fields_json(&kind.state),
        "row":kind.row,"page":page,"operations":kind.operations.iter().map(|(name, op)| (name.clone(), operation_json(op))).collect::<Map<_,_>>()
    })
}

fn fields_json(fields: &BTreeMap<String, Field>) -> Map<String, Value> {
    fields.iter().map(|(name, field)| {
        let mut view = json!({
            "name":name,
            "type":field.field_type.as_ref().map(field_type).unwrap_or_else(|| "computed".into()),
            "required":field.required,
            "computed":field.expression.is_some(),
            "badge":field.is_badge(),
        });
        if let Some(default) = field.default.as_ref().and_then(constant) {
            view["default"] = default;
        }
        (name.clone(), view)
    }).collect()
}

fn field_type(field: &FieldType) -> String {
    use FieldType::*;
    match field {
        Text => "text".into(),
        LongText => "long text".into(),
        Bool => "yes/no".into(),
        Number(units) if units.is_empty() => "number".into(),
        Number(units) => format!("number({})", units.join(", ")),
        Money(Some(currency)) => format!("money({currency})"),
        Money(None) => "money".into(),
        Date => "date".into(),
        Time => "time".into(),
        When => "when".into(),
        Duration => "duration".into(),
        Choice(choices) => format!("choice({})", choices.join(", ")),
        Url => "url".into(),
        Link(target) => format!("link({target})"),
        Person => "person".into(),
        Repeat => "repeat".into(),
        Place => "place".into(),
        File => "file".into(),
        Colour => "colour".into(),
        Data => "data".into(),
        List(item) if **item == Person => "people".into(),
        List(item) => format!("list({})", field_type(item)),
        Records(fields) => format!(
            "records({})",
            fields
                .iter()
                .map(|(name, field)| format!("{name}: {}", field_type(field)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Anchor => "record anchor".into(),
    }
}

fn operation_json(operation: &format::Operation) -> Value {
    use OperationForm::*;
    let none = Vec::new();
    let (form, fields) = match &operation.form {
        Add { fields, .. } => ("add", fields),
        Change(fields) => ("change", fields.as_ref().unwrap_or(&none)),
        Remove => ("remove", &none),
        Duplicate => ("duplicate", &none),
        Set(_) => ("set", &none),
        Open(_) => ("open", &none),
        Timer { .. } => ("timer", &none),
        Code => ("code", &none),
        AppendRecord(fields) => ("append_record", fields),
        ChangeRecord { fields, .. } => ("change_record", fields),
        RemoveRecord => ("remove_record", &none),
    };
    let mut result = json!({
        "name":operation.name,"form":form,"fields":fields,"placement":operation.placements,
        "inputs":fields_json(&operation.inputs)
    });
    match &operation.form {
        Set(assignments) => {
            result["assignments"] = Value::Array(
                assignments
                    .iter()
                    .map(|set| {
                        let target = if set.state {
                            format!("state.{}", set.target)
                        } else {
                            set.target.clone()
                        };
                        if let Some(value) = &set.value {
                            match constant(value) {
                                Some(value) => json!({"target":target,"value":value}),
                                None => json!({"target":target,"computed":true}),
                            }
                        } else {
                            json!({"target":target,"value":"ask"})
                        }
                    })
                    .collect(),
            );
            if let [entry] = result["assignments"]
                .as_array()
                .expect("assignment array")
                .as_slice()
            {
                let entry = entry.clone();
                result["target"] = entry["target"].clone();
                result["value"] = entry["value"].clone();
            }
        }
        ChangeRecord {
            within: Some(within),
            ..
        } => result["within"] = as_json(within),
        _ => {}
    }
    result
}

fn constant(expression: &format::Expression) -> Option<Value> {
    expression.literal().map(as_json)
}
