//! One-time upgrade for the worker-free Notes archive shipped with schema v1.

/// Bind the original packed archive and checksum before adding the opt-in.
/// The new checksum is the CLI's packed checksum with that one change.
pub(super) const LIVE_EDITING: &str = concat!(
    "UPDATE installed_plugins
     SET archive=CAST(json_set(CAST(archive AS TEXT),
         '$.manifest.kinds[0].schema.\"x-calternal-live-editing\"',json('true')) AS BLOB),
         checksum='e986f21d806fcff36483c27ee5bbec91bf991c07713e974866a9d753482cb5d8'
     WHERE id='calternal-notes' AND approval=0
       AND checksum='7b000b042ca62642fefe1cf6e96565dcb0f361bab62052cab8002d7891567f17'
       AND archive=CAST('",
    include_str!("notes_before_live_editing.calplugin"),
    "' AS BLOB);",
);
