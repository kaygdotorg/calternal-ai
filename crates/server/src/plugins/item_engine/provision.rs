//! Kind registrations an activated v2 package needs in a space.
use super::*;

/// Kind registrations a v2 package needs in a space, in the shape
/// [`crate::store::Store::provision_plugin`] applies at activation: placement,
/// file format and whether the kind opts into the live room. Pure; the
/// manifest was validated at install. Cost: O(kinds), at most 64.
pub fn kind_registrations(
    package: &ManifestV2,
) -> Result<Vec<(String, Placement, FileFormat, bool)>, Error> {
    let mut registrations = Vec::new();
    for kind in package.declaration()["kinds"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let placement = match decode::<ManifestPlacement>(&kind["storage"])? {
            ManifestPlacement::Visible { folder } => Placement::Visible { folder },
            ManifestPlacement::VisibleTree {
                folder,
                subdirectories,
            } => Placement::VisibleTree {
                folder,
                subdirectories,
            },
            ManifestPlacement::Hidden { .. } => Placement::Hidden {
                plugin_id: package.id().to_owned(),
            },
        };
        let format = if kind["claimed_text"] == false {
            FileFormat::Opaque {
                extension: decode(&kind["extension"])?,
            }
        } else {
            FileFormat::Markdown
        };
        registrations.push((
            decode(&kind["id"])?,
            placement,
            format,
            kind["live_room"] == "yjs-v1",
        ));
    }
    Ok(registrations)
}
