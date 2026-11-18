//! CLI presentation regression fixtures.

use super::*;
use crate::admin_cli::MachinesCmd;
use calternal_cli::testing::assert_golden;
use calternal_server::store::Store;
use std::path::{Path, PathBuf};
use time::UtcOffset;

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/cli")
        .join(name)
}

/// A paired, stopped installation (as `serve --create` leaves it) with the
/// given pool machines written before recovery is first attached.
fn installation(machines: &[(&str, &str)]) -> (tempfile::TempDir, admin_cli::Installation) {
    use std::os::unix::fs::DirBuilderExt;
    let dir = tempfile::tempdir().unwrap();
    let roots = calternal_roots::Roots {
        appdata: dir.path().join("appdata"),
        userdata_ssd: dir.path().join("userdata-ssd"),
        userdata_hdd: dir.path().join("userdata-hdd"),
    };
    let id = "00000000-0000-0000-0000-000000000001";
    drop(
        Store::open(
            &roots,
            uuid::Uuid::parse_str(id).unwrap(),
            true,
            calternal_content::DEFAULT_RECEIPT_RETENTION,
        )
        .unwrap(),
    );
    let c = calternal_auth::db::open_conn(&roots.appdata.join("system/auth.db")).unwrap();
    for (machine, name) in machines {
        c.execute(
                "INSERT INTO machines(id,owner_account,public_key,name,os,arch,enrolled_ms) VALUES(?1,NULL,?2,?3,'linux','x86_64',0)",
                rusqlite::params![machine, vec![7u8; 32], name],
            )
            .unwrap();
    }
    drop(c);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .recursive(true)
        .create(roots.userdata_ssd.join(".calternal/recovery"))
        .unwrap();
    let installation = admin_cli::Installation {
        appdata: roots.appdata,
        userdata_ssd: roots.userdata_ssd,
        userdata_hdd: roots.userdata_hdd,
        installation_id: id.into(),
    };
    (dir, installation)
}

fn redact_code_and_time(text: &str) -> String {
    let mut cleaned = String::new();
    for line in text.lines() {
        if line.starts_with("code:") {
            cleaned.push_str("code:     <CODE>\n");
        } else if line.starts_with("expires:") {
            cleaned.push_str("expires:  <TIME>\n");
        } else {
            cleaned.push_str(line);
            cleaned.push('\n');
        }
    }
    cleaned
}

#[test]
fn machine_and_plugin_list_goldens_cover_120_80_and_40_columns() {
    let (_keep, installation) = installation(&[(
        "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        "build-machine-with-a-long-name",
    )]);
    for width in [120, 80, 40] {
        let out = Output::fixed_terminal(false, width, UtcOffset::UTC, true);
        let machines = admin_cli::machines(
            MachinesCmd::List {
                installation: installation.clone(),
            },
            &out,
        )
        .unwrap();
        assert_golden(
            &golden(&format!("machines-list-width-{width}.txt")),
            &out.render(&machines),
        );

        let digest = admin_cli::plugin_digest(Path::new("/nonexistent-plugins"));
        assert_golden(
            &golden(&format!("plugin-digest-width-{width}.txt")),
            &out.render(&digest),
        );
    }
}

#[test]
fn no_hints_is_a_global_calternal_option() {
    use clap::Parser;

    let cli = Cli::try_parse_from([
        "calternal",
        "machines",
        "list",
        "--appdata",
        "/tmp/calternal/appdata",
        "--userdata-ssd",
        "/tmp/calternal/ssd",
        "--userdata-hdd",
        "/tmp/calternal/hdd",
        "--installation-id",
        "00000000-0000-0000-0000-000000000001",
        "--no-hints",
    ])
    .unwrap();
    assert!(cli.no_hints);
}

#[test]
fn machine_code_and_empty_list_hint_goldens_cover_enabled_and_disabled_hints() {
    let (_keep, installation) = installation(&[]);
    let out = Output::fixed_terminal(false, 80, UtcOffset::UTC, true);
    let code = admin_cli::machines(
        MachinesCmd::Code {
            installation: installation.clone(),
            pool: true,
            account: None,
            minutes: 60,
        },
        &out,
    )
    .unwrap();
    assert_golden(
        &golden("machines-code-hint.txt"),
        &redact_code_and_time(&out.render(&code)),
    );
    let no_hints = Output::fixed_terminal(false, 80, UtcOffset::UTC, false);
    assert_golden(
        &golden("machines-code-no-hint.txt"),
        &redact_code_and_time(&no_hints.render(&code)),
    );

    let (_keep_empty, empty_installation) = self::installation(&[]);
    let empty = admin_cli::machines(
        MachinesCmd::List {
            installation: empty_installation,
        },
        &out,
    )
    .unwrap();
    assert_golden(&golden("machines-list-empty-hint.txt"), &out.render(&empty));
    assert_golden(
        &golden("machines-list-empty-no-hint.txt"),
        &no_hints.render(&empty),
    );
}

#[test]
fn calternal_command_help_has_examples_and_see_also() {
    use clap::Parser;

    let commands: &[&[&str]] = &[
        &["calternal", "--help"],
        &["calternal", "serve", "--help"],
        &["calternal", "machine", "--help"],
        &["calternal", "machine", "enroll", "--help"],
        &["calternal", "machine", "run", "--help"],
        &["calternal", "machine", "status", "--help"],
        &["calternal", "machines", "--help"],
        &["calternal", "machines", "code", "--help"],
        &["calternal", "machines", "list", "--help"],
        &["calternal", "machines", "revoke", "--help"],
        &["calternal", "machines", "pool-grant", "--help"],
        &["calternal", "plugin-digest", "--help"],
    ];
    for args in commands {
        let help = Cli::try_parse_from(args.iter().copied())
            .err()
            .expect("--help exits before running the command")
            .to_string();
        assert!(help.contains("Examples:"), "{args:?}: {help}");
        assert!(help.contains("See also:"), "{args:?}: {help}");
    }
}
