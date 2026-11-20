//! Extension declarations use the same offline schema registry as ordinary operations.

use super::{catalogue, embedded_schema, entries, str_field};
use calternal_types::{Error, ErrorCode, HarnessRef, Result};
use serde::Serialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::OnceLock};

fn normalize_version(entry: &mut Value) -> std::result::Result<u16, String> {
    let version = entry["version"]
        .as_f64()
        .filter(|n| (1.0..=65535.0).contains(n) && n.fract() == 0.0)
        .ok_or("Invalid extension interface version")? as u16;
    entry["version"] = Value::from(version);
    Ok(version)
}

pub(super) fn check_declarations(manifest: &mut Value) -> std::result::Result<(), String> {
    for app in manifest["apps"].as_array_mut().into_iter().flatten() {
        let mut points = BTreeSet::new();
        for point in app
            .get_mut("extension_points")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            let version = normalize_version(point)?;
            let id = str_field(point, "id");
            if !points.insert((id, version)) {
                return Err(format!("Duplicate extension point {id} version {version}"));
            }
            embedded_schema(&point["input_schema"])?;
            embedded_schema(&point["output_schema"])?;
        }
    }
    let Some(contributions) = manifest.get_mut("contributions") else {
        return Ok(());
    };
    for contribution in contributions.as_array_mut().into_iter().flatten() {
        normalize_version(contribution)?;
    }
    let mut contributions = BTreeSet::new();
    for contribution in entries(manifest, "contributions") {
        let id = str_field(contribution, "id");
        if !contributions.insert(id) {
            return Err(format!("Duplicate contribution {id}"));
        }
        if !entries(manifest, "operations").any(|op| op["id"] == contribution["operation"]) {
            return Err(format!(
                "Contribution {id} must reference an owned operation"
            ));
        }
    }
    Ok(())
}

/// Exact matching extension point and contributor schemas compiled offline.
/// Installed snapshots share this immutable proof across calls.
#[derive(Debug)]
pub struct ContributionInterface {
    input: jsonschema::Validator,
    output: jsonschema::Validator,
}

#[derive(Serialize)]
struct CallEnvelope<'a> {
    request_id: &'a str,
    execution: &'a str,
    contribution: &'a HarnessRef,
    input: &'a Value,
}

#[derive(Serialize)]
struct EntryEnvelope<'a> {
    call: &'a str,
    input: &'a Value,
}

impl ContributionInterface {
    /// Check both parsed schema pairs and compile their common signatures.
    /// Equal schemas resolve references in the same pinned offline registry
    /// and base URI. Refuses incompatible or unresolvable interfaces.
    /// Cost: up to two schema compilations, for installation rather than dispatch.
    pub fn check(point: &Value, operation: &Value) -> std::result::Result<Self, String> {
        for (key, label) in [("input_schema", "input"), ("output_schema", "output")] {
            if point.get(key).is_none() || point[key] != operation[key] {
                return Err(format!(
                    "Contribution {label} schema does not match the extension point"
                ));
            }
        }
        Ok(Self {
            input: embedded_schema(&point["input_schema"])?,
            output: embedded_schema(&point["output_schema"])?,
        })
    }

    /// Validate input against the identical checked signatures on both sides.
    /// Cost: one bounded offline validation, without package code or I/O.
    pub fn accepts_input(&self, data: &Value) -> bool {
        self.input.is_valid(data)
    }

    /// Validate output against the identical checked signatures on both sides.
    /// Cost: one bounded offline validation, without package code or I/O.
    pub fn accepts_output(&self, data: &Value) -> bool {
        self.output.is_valid(data)
    }

    /// Validate a checked harness input before admission can write anything.
    /// Measure both encoded envelopes including LF, using the longest escaped
    /// wire reference for each ID that the host has yet to assign. Returns 413
    /// with the contribution-frame reason when either envelope is too large.
    /// Cost: two bounded encodings and one schema validation. No I/O or wake.
    /// Execution job and alarm admission use this before saving their effects.
    pub fn preflight(&self, harness: &HarnessRef, input: &Value) -> Result<()> {
        static BOUNDS: OnceLock<(usize, String)> = OnceLock::new();
        let (limit, max_ref) = BOUNDS.get_or_init(|| {
            let contract = catalogue();
            let limit = contract["extension_points"]["calls"]["limits"]["encoded_frame_bytes"]
                .as_u64()
                .expect("pinned contribution frame limit");
            let ref_size = contract["$defs"]["Ref"]["maxLength"]
                .as_u64()
                .expect("pinned reference length");
            (limit as usize - 1, "\u{0000}".repeat(ref_size as usize))
        });
        let call = CallEnvelope {
            request_id: max_ref,
            execution: max_ref,
            contribution: harness,
            input,
        };
        let entry = EntryEnvelope {
            call: max_ref,
            input,
        };
        if crate::bounded::check(&call, *limit).is_err()
            || crate::bounded::check(&entry, *limit).is_err()
        {
            return Err(Error::new(
                ErrorCode::TooLarge,
                "Contribution input exceeds the frame limit",
            )
            .with_details(json!({"reason":"contribution_frame_limit"})));
        }
        if !self.accepts_input(input) {
            return Err(Error::input_field(
                "/input",
                "must match the checked contribution interface",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
