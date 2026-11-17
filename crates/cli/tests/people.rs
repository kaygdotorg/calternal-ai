//! Human and machine goldens for the people directory renderer.
use calternal_cli::{
    output::Output,
    people::render,
    testing::{assert_golden, assert_shape},
};
use serde_json::json;
use std::path::Path;
use time::UtcOffset;

#[test]
fn people_output_goldens() {
    let cases = [
        (
            "people-members",
            "calternal people members",
            "spaces.members",
            json!({
                "members":[{"account_id":"01912345-6789-7abc-8123-000000000001","name":"Sam Wu","role":"owner"}],
                "groups":[{"group_id":"01912345-6789-7abc-8123-000000000002","name":"Studio","role":"editor","member_count":3}]
            }),
        ),
        (
            "people-roles",
            "calternal people roles",
            "roles.list",
            json!({
                "roles":[{"name":"editor","capabilities":["content.write","plugins.use","space.read"]}]
            }),
        ),
        (
            "invites-list",
            "calternal invites list",
            "invites.list",
            json!({
                "invites":[{"invite_id":"01912345-6789-7abc-8123-000000000003","role":"reader","created_at":1784283000000_i64,
                    "expires_at":1784887800000_i64,"used_at":null,"revoked_at":null,"link":"https://example.test/invites/example"}]
            }),
        ),
        (
            "groups-list",
            "calternal groups list",
            "groups.list",
            json!({
                "groups":[{"group_id":"01912345-6789-7abc-8123-000000000002","name":"Studio","source":"local",
                    "members":["01912345-6789-7abc-8123-000000000001"]}]
            }),
        ),
        (
            "people-list",
            "calternal people list",
            "people.list",
            json!({
                "people":[{"account_id":"01912345-6789-7abc-8123-000000000001","name":"Sam Wu","role":"admin","disabled":false}]
            }),
        ),
        (
            "people-set-profile",
            "calternal people set-profile",
            "account.set_profile",
            json!({
                "account_id":"01912345-6789-7abc-8123-000000000001","name":"Sam Wu"
            }),
        ),
        (
            "people-add",
            "calternal people add",
            "spaces.add_member",
            json!({}),
        ),
        (
            "people-disable",
            "calternal people disable",
            "people.disable",
            json!({
                "account_id":"01912345-6789-7abc-8123-000000000001","disabled":true
            }),
        ),
        (
            "groups-create",
            "calternal groups create",
            "groups.create",
            json!({
                "group_id":"01912345-6789-7abc-8123-000000000002"
            }),
        ),
        (
            "invites-create",
            "calternal invites create",
            "invites.create",
            json!({
                "invite_id":"01912345-6789-7abc-8123-000000000003","expires_at":1784887800000_i64
            }),
        ),
        (
            "invites-revoke",
            "calternal invites revoke",
            "invites.revoke",
            json!({
                "invite_id":"01912345-6789-7abc-8123-000000000003"
            }),
        ),
        (
            "invites-redeem",
            "calternal invites redeem",
            "invites.redeem",
            json!({
                "space_id":"01912345-6789-7abc-8123-000000000004"
            }),
        ),
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    for (slug, command, op, data) in cases {
        let machine = Output::fixed(true, None, UtcOffset::UTC);
        let printed = machine.render(&render(&machine, op, "people-example", &data).unwrap());
        assert_shape(command, &serde_json::from_str(&printed).unwrap());
        assert_golden(&root.join(format!("{slug}.json")), &printed);
        for width in [40, 80, 120] {
            let human = Output::fixed_terminal(false, width, UtcOffset::UTC, true);
            let printed = human.render(&render(&human, op, "people-example", &data).unwrap());
            assert_golden(&root.join(format!("{slug}-width-{width}.txt")), &printed);
        }
    }
}
