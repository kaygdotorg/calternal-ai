//! Plain-language permissions and entry points in the v2 install review.
use super::{CliError, Reply, package_error};
use calternal_plugin_host::ManifestV2;
use calternal_server::plugins::package_store::PackageV2;
use serde_json::{Value, json};

pub(super) fn review(package: &PackageV2, manifest: &ManifestV2) -> Result<Reply, CliError> {
    let checksum = package.checksum().map_err(package_error)?;
    let data = manifest.declaration();
    let names = |field: &str, id: &str| {
        data[field]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                if id.is_empty() {
                    entry.as_str()
                } else {
                    entry[id].as_str()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let operations = data["operations"].as_array().into_iter().flatten();
    let methods: Vec<_> = operations
        .clone()
        .flat_map(|op| op["methods"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let operations: Vec<_> = operations
        .map(|operation| {
            json!({
                "id":operation["id"], "risk":operation["risk"],
                "capability":operation["required_capability"],
            })
        })
        .collect();
    let rows = operations
        .iter()
        .map(|op| {
            ["id", "risk", "capability"]
                .into_iter()
                .map(|field| op[field].as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .collect();
    let secrets: Vec<_> = data["secrets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|secret| secret["name"].as_str())
        .collect();
    let lines = network_summary(data);
    let routes = ids(data, "credential_routes")
        .map(|route| {
            let connection = &route["connection"];
            let destination = if let Some(host) = connection["fixed_site"].as_str() {
                format!("HTTPS {host}")
            } else {
                format!("server you add ({})", text(connection, "user_server"))
            };
            vec![
                text(route, "id").to_owned(),
                text(route, "secret").to_owned(),
                destination,
            ]
        })
        .collect::<Vec<_>>();
    let jobs = labels(data, "jobs", "run_operation");
    let schedules = labels(data, "schedule_eligibility", "operation");
    let webhooks = labels(data, "wait_webhooks", "schema_operation");
    let mut reply = Reply::new(json!({
        "id":manifest.id(), "api_version":2, "version":data["version"],
        "checksum":checksum, "network_summary":lines,
        "operations":operations, "host_calls":methods, "secrets":secrets,
        "complete_manifest":data, "changes":null,
        "credential_route_summary":routes, "jobs":jobs,
        "schedules":schedules, "wait_webhooks":webhooks,
    }))
    .fields([
        ("Plugin", manifest.id().to_owned()),
        (
            "Version",
            data["version"].as_str().unwrap_or_default().to_owned(),
        ),
        ("SHA-256", checksum),
        ("Item kinds", names("kinds", "id")),
        ("Can read", names("reads", "")),
        ("Host calls", methods.join(", ")),
        ("Settings", names("settings", "key")),
        ("Secrets requested", secrets.join(", ")),
        ("Providers", names("providers", "id")),
    ])
    .table(
        &["OPERATION", "RISK", "CAPABILITY"],
        rows,
        "No operation permissions requested.",
    );
    if !routes.is_empty() {
        reply = reply.table(&["CREDENTIAL ROUTE", "SECRET", "DESTINATION"], routes, "");
    }
    for (heading, declared) in [
        ("Jobs", jobs),
        ("Schedules", schedules),
        ("Inbound webhooks", webhooks),
    ] {
        if !declared.is_empty() {
            reply = reply.fields([(heading, declared.join(", "))]);
        }
    }
    for line in lines {
        reply = reply.line(line);
    }
    Ok(reply.hint("Pass the reviewed SHA-256 to `calternal plugin install --approve` to register this package."))
}

fn network_summary(manifest: &Value) -> Vec<String> {
    let network = &manifest["network"];
    let mut summary = Vec::new();
    if let Some(sites) = network["fixed_sites"].as_array() {
        let sites: Vec<_> = sites.iter().filter_map(Value::as_str).collect();
        summary.push(format!(
            "Can connect to these HTTPS sites: {}",
            sites.join(", ")
        ));
    }
    if network["user_servers"] == true {
        summary.push("Can connect to servers you add.".into());
    }
    if network["any_website"] == true {
        summary.push("Can connect to any website.".into());
    }
    if manifest["content_fetch"] == true {
        summary.push(
            if network == "none" {
                "No plugin network access. Loads remote images anonymously."
            } else {
                "Loads remote images anonymously."
            }
            .into(),
        );
    }
    if summary.is_empty() {
        summary.push("No network access".into());
    }
    summary
}

fn ids<'a>(value: &'a Value, field: &str) -> impl Iterator<Item = &'a Value> {
    value[field].as_array().into_iter().flatten()
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().unwrap_or_default()
}

fn labels(data: &Value, field: &str, operation: &str) -> Vec<String> {
    ids(data, field)
        .map(|value| format!("{} via {}", text(value, "id"), text(value, operation)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_cli::{InstallArgs, install};
    use calternal_cli::output::Output;

    fn manifest() -> Value {
        json!({"api_version":2,"id":"fixture","name":"Fixture","version":"1.0.0",
            "icon":"book","kinds":[],"apps":[],"operations":[],"network":"none"})
    }

    #[test]
    fn v2_installer_reports_all_network_tiers_and_content_fetch() {
        for (permission, content_fetch, line) in [
            (json!("none"), false, "No network access"),
            (
                json!("none"),
                true,
                "No plugin network access. Loads remote images anonymously.",
            ),
            (
                json!({"fixed_sites":["read.example.org"]}),
                false,
                "Can connect to these HTTPS sites: read.example.org",
            ),
            (
                json!({"user_servers":true}),
                false,
                "Can connect to servers you add.",
            ),
            (
                json!({"any_website":true}),
                false,
                "Can connect to any website.",
            ),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let mut manifest = manifest();
            manifest["network"] = permission;
            if content_fetch {
                manifest["content_fetch"] = true.into();
            }
            std::fs::write(directory.path().join("manifest.json"), manifest.to_string()).unwrap();
            let reply = crate::plugin_cli::review(directory.path(), None).unwrap();
            let human = Output::detect_with_options(false, true).render(&reply);
            assert!(human.lines().any(|out| out == line), "{human}");
            let output = Output::detect_with_options(true, true).render(&reply);
            let json: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(json["data"]["network_summary"], serde_json::json!([line]));
        }
    }

    #[test]
    fn v2_installer_discloses_credentials_and_job_entry_points() {
        let directory = tempfile::tempdir().unwrap();
        let mut manifest = manifest();
        manifest["network"] = json!({"fixed_sites":["read.example.org"],"user_servers":true});
        manifest["operations"] = json!([{"id":"fixture.read","title":"Read", "input_schema":{},"output_schema":{},
            "required_capability":"space.read", "risk":"read","idempotency":"read_only","methods":[]}]);
        manifest["jobs"] = json!([{"id":"job","run_operation":"fixture.read","replayable":true}]);
        manifest["schedule_eligibility"] = json!([{"id":"calendar","operation":"fixture.read"}]);
        manifest["wait_webhooks"] =
            json!([{"id":"notify","schema_operation":"fixture.read","signing_secret":"login"}]);
        manifest["secrets"] = json!([{"name":"login","label":"Login","purpose":"Review data","scope":"personal","required":true}]);
        manifest["credential_routes"] = json!([
            {"id":"fixed","connection":{"fixed_site":"read.example.org"},"profile":"test","secret":"login","input_schema":{},"output_schema":{}},
            {"id":"added","connection":{"user_server":"reading"},"profile":"test","secret":"login","input_schema":{},"output_schema":{}}
        ]);
        let worker = b"literal example";
        let digest = ring::digest::digest(&ring::digest::SHA256, worker);
        manifest["worker"] = json!({"path":"worker","sha256":hex::encode(digest.as_ref())});
        std::fs::write(directory.path().join("worker"), worker).unwrap();
        std::fs::write(directory.path().join("manifest.json"), manifest.to_string()).unwrap();
        let reply = crate::plugin_cli::review(directory.path(), None).unwrap();
        let human = Output::detect_with_options(false, true).render(&reply);
        for text in [
            "CREDENTIAL ROUTE",
            "HTTPS read.example.org",
            "server you add (reading)",
            "login",
            "job via fixture.read",
            "calendar via fixture.read",
            "notify via fixture.read",
        ] {
            assert!(human.contains(text), "{human}");
        }
    }

    #[test]
    fn v2_installer_refuses_update_and_placement_flags_before_socket_access() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("manifest.json"),
            manifest().to_string(),
        )
        .unwrap();
        for (update, space, account) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
            (false, true, true),
        ] {
            let fixed_uuid = "0193aa00-0000-7000-8000-000000000087";
            let arguments = InstallArgs {
                source: directory.path().to_owned(),
                appdata: directory.path().join("missing-appdata"),
                approve: "approval".into(),
                space: space.then(|| fixed_uuid.parse().unwrap()),
                account: account.then(|| fixed_uuid.parse().unwrap()),
            };
            let error = install(arguments, update).unwrap_err();
            assert!(format!("{error:?}").contains("Updates and space enablement are unavailable"));
        }
    }
}
