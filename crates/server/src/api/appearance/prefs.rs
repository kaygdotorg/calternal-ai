//! Browser-compatible defaults, bounded patches and role traversal.
use calternal_core::*;
use calternal_types::Error;
use rusqlite::{Connection, OptionalExtension, params};

pub(super) const ROLES: [FontRole; 5] = [
    FontRole::Ui,
    FontRole::Chat,
    FontRole::Code,
    FontRole::Terminal,
    FontRole::Brand,
];

pub(super) fn role(fonts: &AppearanceFonts, role: FontRole) -> &FontChoice {
    match role {
        FontRole::Ui => &fonts.ui,
        FontRole::Chat => &fonts.chat,
        FontRole::Code => &fonts.code,
        FontRole::Terminal => &fonts.terminal,
        FontRole::Brand => &fonts.brand,
    }
}

pub(super) fn role_mut(fonts: &mut AppearanceFonts, role: FontRole) -> &mut FontChoice {
    match role {
        FontRole::Ui => &mut fonts.ui,
        FontRole::Chat => &mut fonts.chat,
        FontRole::Code => &mut fonts.code,
        FontRole::Terminal => &mut fonts.terminal,
        FontRole::Brand => &mut fonts.brand,
    }
}

pub(super) fn bundled(choice: &FontChoice) -> bool {
    choice.id.is_none()
        && matches!(
            (choice.source, choice.family.as_str()),
            (FontSource::System, "System")
                | (FontSource::Google, "Google Sans Flex" | "Dancing Script")
                | (FontSource::Fontsource, "Cascadia Code" | "Maple Mono")
        )
}

fn default_choice(role: FontRole) -> FontChoice {
    let (source, family) = match role {
        FontRole::Ui | FontRole::Chat => (FontSource::Google, "Google Sans Flex"),
        FontRole::Code | FontRole::Terminal => (FontSource::Fontsource, "Cascadia Code"),
        FontRole::Brand => (FontSource::Google, "Dancing Script"),
    };
    FontChoice {
        family: family.into(),
        source,
        id: None,
    }
}

fn defaults() -> AppearancePreferences {
    AppearancePreferences {
        wallpaper_id: Some("auto".into()),
        materials: AppearanceMaterials {
            blur: true,
            dim: true,
            tint: true,
        },
        theme_id: None,
        preset_id: Some("calternal".into()),
        accent: AppearanceAccent::Wallpaper,
        accent_color: "#3b6ea8".into(),
        brand: "Calternal".into(),
        stream_text: false,
        fonts: AppearanceFonts {
            ui: default_choice(FontRole::Ui),
            chat: default_choice(FontRole::Chat),
            code: default_choice(FontRole::Code),
            terminal: default_choice(FontRole::Terminal),
            brand: default_choice(FontRole::Brand),
        },
        motion: true,
    }
}

pub(super) fn get(c: &Connection, account: &str) -> Result<AppearancePreferences, Error> {
    c.query_row(
        "SELECT prefs FROM appearance WHERE account=?1",
        [account],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(super::db_error)?
    .map_or_else(
        || Ok(defaults()),
        |raw| serde_json::from_str(&raw).map_err(super::db_error),
    )
}

pub(super) fn reset_family(prefs: &mut AppearancePreferences, id: &str) {
    for role in ROLES {
        let choice = role_mut(&mut prefs.fonts, role);
        if choice.id.as_deref() == Some(id) {
            *choice = default_choice(role);
        }
    }
}

pub(super) fn merge(
    prefs: &mut AppearancePreferences,
    patch: AppearancePatch,
) -> Result<(), Error> {
    macro_rules! apply {
        ($target:expr, $patch:ident, $($field:ident),+ $(,)?) => {
            $(if let Some(value) = $patch.$field { $target.$field = value; })+
        };
    }
    apply!(
        prefs,
        patch,
        wallpaper_id,
        theme_id,
        preset_id,
        accent,
        accent_color,
        brand,
        stream_text,
        motion,
    );
    if let Some(materials) = patch.materials {
        apply!(prefs.materials, materials, blur, dim, tint);
    }
    if let Some(fonts) = patch.fonts {
        apply!(prefs.fonts, fonts, ui, chat, code, terminal, brand);
    }
    let valid_id =
        |id: &str| !id.is_empty() && id.len() <= 160 && !id.chars().any(char::is_control);
    if prefs.brand.chars().count() > 40
        || prefs.brand.chars().any(char::is_control)
        || prefs.accent_color.len() != 7
        || !prefs
            .accent_color
            .strip_prefix('#')
            .is_some_and(|c| c.bytes().all(|b| b.is_ascii_hexdigit()))
        || [&prefs.wallpaper_id, &prefs.theme_id, &prefs.preset_id]
            .into_iter()
            .flatten()
            .any(|id| !valid_id(id))
    {
        return Err(Error::invalid_input(
            "appearance preference is outside its allowed range",
        ));
    }
    Ok(())
}

pub(super) fn save(
    c: &Connection,
    account: &str,
    prefs: &AppearancePreferences,
) -> Result<(), Error> {
    c.execute(
        "INSERT INTO appearance VALUES(?1,?2)
        ON CONFLICT(account) DO UPDATE SET prefs=excluded.prefs",
        params![
            account,
            serde_json::to_string(prefs).map_err(super::db_error)?
        ],
    )
    .map_err(super::db_error)?;
    c.execute("DELETE FROM font_pins WHERE account=?1", [account])
        .map_err(super::db_error)?;
    for f in ROLES.into_iter().map(|r| role(&prefs.fonts, r)) {
        if bundled(f) {
            continue;
        }
        let id =
            f.id.as_deref()
                .ok_or_else(|| Error::invalid_input("select a local font family"))?;
        let stored = super::db::family(c, account, id)?.ok_or_else(super::missing)?;
        if stored.family != f.family || stored.source != f.source {
            return Err(Error::invalid_input(
                "font choice does not match its local family",
            ));
        }
        if stored.source != FontSource::Upload {
            c.execute(
                "INSERT OR IGNORE INTO font_pins VALUES(?1,?2)",
                params![account, id],
            )
            .map_err(super::db_error)?;
        }
    }
    Ok(())
}
