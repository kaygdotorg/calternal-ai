use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Diagnostic, Field,
    expr::{Node, NodeKind, Op},
};

pub(crate) fn computed_order(fields: &BTreeMap<String, Field>) -> Result<Vec<String>, Diagnostic> {
    fn visit(
        name: &str,
        fields: &BTreeMap<String, Field>,
        pending: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        output: &mut Vec<String>,
    ) -> Result<(), Diagnostic> {
        if visited.contains(name) {
            return Ok(());
        }
        let Some(field) = fields.get(name) else {
            return Ok(());
        };
        let Some(expression) = &field.expression else {
            return Ok(());
        };
        if pending.len() >= 64 || !pending.insert(name.into()) {
            return Err(field.site.error(format!(
                "Computed fields have a cycle or excessive dependency depth at {name}"
            )));
        }
        let mut dependencies = BTreeSet::new();
        dependencies_of(&expression.root, &mut dependencies, false, false)?;
        for dependency in dependencies {
            visit(dependency, fields, pending, visited, output)?
        }
        pending.remove(name);
        visited.insert(name.into());
        output.push(name.into());
        Ok(())
    }
    let mut output = Vec::new();
    let mut pending = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for name in fields.keys() {
        visit(name, fields, &mut pending, &mut visited, &mut output)?
    }
    Ok(output)
}

fn dependencies_of<'a>(
    node: &'a Node,
    output: &mut BTreeSet<&'a str>,
    scoped: bool,
    direct_time: bool,
) -> Result<(), Diagnostic> {
    let visit = |node: &'a Node, output: &mut BTreeSet<&'a str>| {
        dependencies_of(node, output, scoped, false)
    };
    match &node.kind {
        NodeKind::Name(name) if !scoped => {
            output.insert(name);
        }
        NodeKind::Member(base, field) if matches!(&base.kind, NodeKind::Name(name) if name == "this") =>
        {
            output.insert(field);
        }
        NodeKind::Member(base, _) | NodeKind::Unary(_, base) => visit(base, output)?,
        NodeKind::Call(name, _) if name == "now" && !direct_time => {
            return Err(node
                .site
                .error("In a computed field put now() directly on one side of a comparison"));
        }
        NodeKind::Call(name, args) => {
            let conditional_list = matches!(
                name.as_str(),
                "count" | "sum" | "average" | "min" | "max" | "from"
            );
            for (index, arg) in args.iter().enumerate() {
                dependencies_of(
                    arg,
                    output,
                    scoped || (conditional_list && index == 1),
                    false,
                )?
            }
        }
        NodeKind::List(nodes) => {
            for node in nodes {
                visit(node, output)?
            }
        }
        NodeKind::Binary(op, a, b) => {
            let comparison = matches!(op, Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge);
            let is_now =
                |node: &Node| matches!(&node.kind, NodeKind::Call(name, _) if name == "now");
            dependencies_of(a, output, scoped, comparison && !is_now(b))?;
            dependencies_of(b, output, scoped, comparison && !is_now(a))?;
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn refusal_condition(
    node: &Node,
    kind: &crate::Kind,
    op: &crate::Operation,
) -> Result<(), Diagnostic> {
    let validate = |node| refusal_condition(node, kind, op);
    match &node.kind {
        NodeKind::Value(_) => {}
        NodeKind::Name(name) => {
            if !op.inputs.contains_key(name)
                && kind
                    .fields
                    .get(name)
                    .is_none_or(|field| field.field_type.is_none())
            {
                return Err(node.site.error(format!("The refusal cannot read {name} from the authorized inputs or stored target fields")));
            }
        }
        NodeKind::Member(_, _) => {
            return Err(node
                .site
                .error("Refusal conditions cannot navigate links, state or settings"));
        }
        NodeKind::Call(name, args) => {
            if matches!(name.as_str(), "from" | "links to" | "records after") {
                return Err(node
                    .site
                    .error("Refusal conditions cannot traverse data beyond the target call"));
            }
            for arg in args {
                validate(arg)?
            }
        }
        NodeKind::List(items) => {
            for item in items {
                validate(item)?
            }
        }
        NodeKind::Unary(_, node) => validate(node)?,
        NodeKind::Binary(_, left, right) => {
            validate(left)?;
            validate(right)?;
        }
    }
    Ok(())
}

pub(super) fn mark_viewer_fields(
    fields: &mut BTreeMap<String, Field>,
    order: &[String],
    settings: &BTreeMap<String, Field>,
) {
    for name in order {
        let expression = &fields[name]
            .expression
            .as_ref()
            .expect("a computed field")
            .root;
        let dependent = viewer_dependent(expression, fields, settings);
        fields
            .get_mut(name)
            .expect("a compiled field")
            .viewer_dependent = dependent;
    }
}

fn viewer_dependent(
    node: &Node,
    fields: &BTreeMap<String, Field>,
    settings: &BTreeMap<String, Field>,
) -> bool {
    let depends = |node| viewer_dependent(node, fields, settings);
    match &node.kind {
        NodeKind::Name(name) => {
            name == "open item" || fields.get(name).is_some_and(Field::is_per_viewer)
        }
        NodeKind::Member(base, field) => match &base.kind {
            NodeKind::Name(name) if name == "state" => true,
            NodeKind::Name(name) if name == "settings" => {
                settings.get(field).is_none_or(|setting| setting.per_person)
            }
            NodeKind::Name(name) if name == "this" => {
                fields.get(field).is_some_and(Field::is_per_viewer)
            }
            NodeKind::Name(name) if name == "records" => false,
            NodeKind::Name(name) => {
                !fields.get(name).is_some_and(|field| {
                    matches!(
                        field.field_type,
                        Some(crate::FieldType::Data | crate::FieldType::Records(_))
                    )
                }) || depends(base)
            }
            _ => true,
        },
        NodeKind::Unary(_, value) => depends(value),
        NodeKind::Binary(_, a, b) => depends(a) || depends(b),
        NodeKind::Call(name, values) => {
            matches!(name.as_str(), "from" | "links to" | "records after")
                || values.iter().any(depends)
        }
        NodeKind::List(values) => values.iter().any(depends),
        NodeKind::Value(_) => false,
    }
}
