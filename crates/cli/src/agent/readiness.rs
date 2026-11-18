//! Terminal and JSON output for the cached machine readiness operation.
use crate::output::{Output, Reply};
use serde_json::Value;

/// Render account and pool readiness using terminal-width-aware columns.
/// Cost is linear in the returned machines and provider observations.
pub fn reply(out: &Output, value: Value) -> Reply {
    let rows = ["machines", "pool"]
        .into_iter()
        .flat_map(|group| {
            value[group]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(move |machine| {
                    let name = machine["name"].as_str().unwrap_or("Cloud pool");
                    let id = machine["machine_id"].as_str().unwrap_or("—");
                    let connection = if machine["connected"] == true {
                        "Connected"
                    } else {
                        "Offline"
                    };
                    let execution = if machine["sandbox_fallback"] == "kvm_unavailable" {
                        "Containers only (KVM unavailable)"
                    } else {
                        machine["execution"].as_str().unwrap_or("—")
                    };
                    let last_seen = machine["last_seen_ms"]
                        .as_i64()
                        .map(|ms| out.local_ms(ms))
                        .unwrap_or_else(|| "—".into());
                    let mut base = vec![
                        name.to_owned(),
                        connection.to_owned(),
                        execution.to_owned(),
                        if machine["available"] == true {
                            "Available"
                        } else {
                            "Unavailable"
                        }
                        .to_owned(),
                        id.to_owned(),
                        last_seen,
                    ];
                    let Some(providers) =
                        machine["providers"].as_object().filter(|v| !v.is_empty())
                    else {
                        base.extend(["—".into(), "Unknown".into(), "Unknown".into()]);
                        return vec![base];
                    };
                    providers
                        .iter()
                        .map(|(id, provider)| {
                            let check = &provider["check"];
                            let status = if check.is_null() {
                                "Unknown"
                            } else if check["installed"] == false {
                                "Uninstalled"
                            } else if check["signed_in"] == true {
                                "Signed in"
                            } else {
                                "Signed out"
                            };
                            let mut row = base.clone();
                            let available = if provider["enabled"] == false {
                                "Disabled"
                            } else if provider["ready"] == true {
                                "Ready"
                            } else {
                                "Unavailable"
                            };
                            row.extend([id.clone(), status.into(), available.into()]);
                            row
                        })
                        .collect()
                })
        })
        .collect();
    let commands: Vec<_> = ["machines", "pool"]
        .into_iter()
        .flat_map(|group| value[group].as_array().into_iter().flatten())
        .flat_map(|machine| {
            let name = machine["name"].as_str().unwrap_or("Cloud pool");
            machine["providers"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(move |(id, provider)| {
                    let check = &provider["check"];
                    (check["installed"] == true && check["signed_in"] == false)
                        .then(|| check["sign_in_command"].as_str())
                        .flatten()
                        .map(|command| (format!("{name} · {id}"), command.to_owned()))
                })
        })
        .collect();
    let mut reply = Reply::new(value)
        .table(
            &[
                "Machine",
                "Connection",
                "Execution",
                "Capacity",
                "Machine ID",
                "Last seen",
                "Provider",
                "Sign-in",
                "Readiness",
            ],
            rows,
            "No machines are available to this account in this space.",
        )
        .hint("Use calternal agent check --help to refresh provider sign-in status.");
    if !commands.is_empty() {
        reply = reply.blank().line("Sign-in commands").fields(commands);
    }
    reply
}
