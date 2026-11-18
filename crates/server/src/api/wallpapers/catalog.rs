//! Fixed catalog and rendition table shared by HTTP admission and the cache.

use calternal_core::{WallpaperCredit, WallpapersListResult};
use serde_json::Value;
use std::sync::LazyLock;

pub(super) const DEFAULT: &str = "EX_STserBZM";
pub(super) const PHOTOS: [(&str, &str, &str); 8] = [
    ("fbV03MdVtJA", "Annie Spratt", "anniespratt"),
    ("BnOdWOMW7Is", "Bruce Barrow", "bruceb_uk"),
    ("Nkwrpa_v0WA", "Dave Hoefler", "iamthedave"),
    ("B8zfbse7mOc", "Eugenia Pan'kiv", "eugenivy_now"),
    ("_zDcx4dD7L0", "Felix Yu", "qixna"),
    (DEFAULT, "Geronimo Giqueaux", "ggiqueaux"),
    ("ngWVYP9NIYM", "Geronimo Giqueaux", "ggiqueaux"),
    ("I_xBR4NmDhU", "Nataliya Melnychuk", "natinati"),
];

pub(super) static LIST: LazyLock<Value> = LazyLock::new(|| {
    const UTM: &str = "utm_source=calternal&utm_medium=referral";
    serde_json::to_value(WallpapersListResult {
        default_id: DEFAULT.to_owned(),
        wallpapers: PHOTOS
            .iter()
            .map(|&(id, author, handle)| WallpaperCredit {
                id: id.to_owned(),
                author: author.to_owned(),
                author_url: format!("https://unsplash.com/@{handle}?{UTM}"),
                link: format!("https://unsplash.com/photos/{id}?{UTM}"),
            })
            .collect(),
    })
    .expect("wallpaper catalog serializes")
});

#[derive(Clone, Copy)]
pub(super) struct Rendition {
    pub name: &'static str,
    pub width: u16,
    pub quality: u8,
    pub blur: Option<u8>,
    pub light: bool,
    pub max_bytes: usize,
}

impl Rendition {
    const fn screen(name: &'static str, light: bool) -> Self {
        Self {
            name,
            width: 1600,
            quality: 72,
            blur: None,
            light,
            max_bytes: 2 * 1024 * 1024,
        }
    }

    const fn startup(name: &'static str, density: u8, light: bool) -> Self {
        Self {
            name,
            width: 96 * density as u16,
            quality: 50,
            blur: Some(40 * density),
            light,
            max_bytes: 128 * 1024,
        }
    }
}

pub(super) const RENDITIONS: [Rendition; 9] = [
    Rendition::screen("full", false),
    Rendition::screen("light", true),
    Rendition {
        name: "thumb",
        width: 360,
        quality: 70,
        blur: None,
        light: false,
        max_bytes: 256 * 1024,
    },
    Rendition::startup("blur", 1, false),
    Rendition::startup("blur-2x", 2, false),
    Rendition::startup("blur-3x", 3, false),
    Rendition::startup("blur-light", 1, true),
    Rendition::startup("blur-light-2x", 2, true),
    Rendition::startup("blur-light-3x", 3, true),
];

pub(super) fn find(id: &str, name: &str) -> Option<(usize, Rendition)> {
    let slot = PHOTOS.iter().position(|&(photo, _, _)| photo == id)?;
    let r = RENDITIONS.iter().copied().find(|r| r.name == name)?;
    (r.blur.is_none() || id == DEFAULT).then_some((slot, r))
}

/// Keep public admission aligned with the canonical rendition table.
pub(super) fn public(id: &str, name: &str) -> bool {
    id == DEFAULT && find(id, name).is_some_and(|(_, r)| r.blur.is_some())
}
