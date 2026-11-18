//! Guarded adapters for Google Fonts, Fontsource and Fontshare.
use super::{
    db::{FAMILY_BYTES, RemoteInstall},
    face::{Face, Format, unicode_range},
};
use calternal_core::{FontCatalogueFamily, FontSource};
use calternal_types::Error;
use reqwest::{Client, Url};
use serde::{Deserialize, de::DeserializeOwned};
use std::collections::{BTreeMap, HashMap};
mod css;

const GOOGLE_LIST: &str = "https://fonts.google.com/metadata/fonts";
const GOOGLE_CSS: &str = "https://fonts.googleapis.com/css2";
const FONTSOURCE: &str = "https://api.fontsource.org/v1";
const FONTSHARE: &str = "https://api.fontshare.com/v2";
const GOOGLE_BYTES: &str = "fonts.gstatic.com";
const FONTSOURCE_BYTES: &str = "cdn.jsdelivr.net";
const FONTSHARE_BYTES: &str = "cdn.fontshare.com";
const HOSTS: &[&str] = &[
    "fonts.google.com",
    "fonts.googleapis.com",
    GOOGLE_BYTES,
    "api.fontsource.org",
    FONTSOURCE_BYTES,
    "api.fontshare.com",
    FONTSHARE_BYTES,
];
const BROWSER: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const CATALOGUE_CAP: usize = 8 * 1024 * 1024;
const DESCRIPTOR_CAP: usize = 1024 * 1024;
const MAX_FACES: usize = 512;

#[derive(Clone)]
pub(super) struct Entry {
    pub row: FontCatalogueFamily,
    pub folded: String,
    key: String,
    weights: Vec<u16>,
    variable: Option<(u16, u16)>,
    variable_selector: Option<String>,
    attribution: String,
}

fn unavailable() -> Error {
    Error::unavailable("font provider is unavailable or supplied invalid font data")
}

pub(super) fn client() -> Result<Client, Error> {
    crate::netguard::client(HOSTS)
        .map_err(|_| Error::internal("could not initialize font downloads"))
}

async fn get(client: &Client, url: Url, cap: usize) -> Result<Vec<u8>, Error> {
    let response = crate::netguard::get_with_redirects(
        url,
        0,
        async |_| Ok(()),
        |url| client.get(url).header(reqwest::header::USER_AGENT, BROWSER),
    )
    .await
    .map_err(|_| unavailable())?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(unavailable());
    }
    crate::netguard::read_bounded(response, cap)
        .await
        .map_err(|_| unavailable())
}

fn fixed_url(text: &str) -> Url {
    Url::parse(text).expect("validated provider endpoint")
}

async fn json<T: DeserializeOwned>(client: &Client, url: &str, cap: usize) -> Result<T, Error> {
    let bytes = get(client, fixed_url(url), cap).await?;
    let bytes = bytes.strip_prefix(b")]}'").unwrap_or(&bytes);
    serde_json::from_slice(bytes).map_err(|_| unavailable())
}

fn key_valid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn entry(
    source: FontSource,
    key: String,
    family: String,
    category: String,
    weights: Vec<u16>,
    variable: Option<(u16, u16)>,
    variable_selector: Option<String>,
    attribution: String,
) -> Option<Entry> {
    if family.is_empty()
        || family.chars().count() > 120
        || family.chars().any(char::is_control)
        || (source != FontSource::Google && !key_valid(&key))
    {
        return None;
    }
    let raw = category
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let category = match raw.as_str() {
        "sans serif" | "sans-serif" | "sans" => "Sans Serif",
        "mono" | "monospace" => "Monospace",
        "slab" | "serif" => "Serif",
        "handwriting" | "handwritten" | "script" => "Handwriting",
        "display" => "Display",
        _ => "Other",
    };
    let prefix = match source {
        FontSource::Google => "google",
        FontSource::Fontsource => "fontsource",
        FontSource::Fontshare => "fontshare",
        _ => return None,
    };
    let folded = family.to_lowercase();
    let id = format!("{prefix}-{}", crate::store::sha256_hex(key.as_bytes()));
    Some(Entry {
        row: FontCatalogueFamily {
            id,
            family,
            category: category.into(),
            source,
            mono: category == "Monospace",
        },
        folded,
        key,
        weights,
        variable,
        variable_selector,
        attribution,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleCatalogue {
    family_metadata_list: Vec<GoogleFamily>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleFamily {
    family: String,
    category: String,
    #[serde(default)]
    popularity: u32,
    #[serde(default)]
    is_open_source: bool,
    #[serde(default)]
    fonts: BTreeMap<String, serde::de::IgnoredAny>,
    #[serde(default)]
    axes: Vec<GoogleAxis>,
}
#[derive(Deserialize)]
struct GoogleAxis {
    tag: String,
    min: f64,
    max: f64,
}

#[derive(Deserialize)]
struct FontsourceFamily {
    id: String,
    family: String,
    category: String,
    #[serde(rename = "type")]
    font_type: String,
    #[serde(default)]
    license: String,
}

#[derive(Deserialize)]
struct ShareCatalogue {
    fonts: Vec<ShareFamily>,
}
#[derive(Deserialize)]
struct ShareFamily {
    name: String,
    slug: String,
    category: String,
    #[serde(default)]
    styles: Vec<ShareStyle>,
    #[serde(default)]
    license_type: String,
}
#[derive(Deserialize)]
struct ShareStyle {
    #[serde(default)]
    is_italic: bool,
    #[serde(default)]
    is_variable: bool,
    weight: ShareWeight,
}
#[derive(Deserialize)]
struct ShareWeight {
    weight: u16,
    number: u16,
}

fn weight(n: u16) -> bool {
    (1..=1000).contains(&n)
}

fn range(lo: f64, hi: f64) -> Option<(u16, u16)> {
    (lo.is_finite() && hi.is_finite() && lo >= 1.0 && hi <= 1000.0 && lo < hi)
        .then(|| (lo.ceil() as u16, hi.floor() as u16))
}

pub(super) async fn catalogue(client: &Client, index: usize) -> Result<Vec<Entry>, Error> {
    let entries: Vec<Entry> = match index {
        0 => {
            let mut rows = json::<GoogleCatalogue>(client, GOOGLE_LIST, CATALOGUE_CAP)
                .await?
                .family_metadata_list;
            rows.sort_by_key(|r| r.popularity);
            rows.into_iter()
                .filter(|row| row.is_open_source)
                .filter_map(|row| {
                    let variable = row
                        .axes
                        .iter()
                        .find(|a| a.tag == "wght")
                        .and_then(|a| range(a.min, a.max));
                    let weights = row
                        .fonts
                        .keys()
                        .filter_map(|key| key.parse::<u16>().ok())
                        .collect();
                    entry(
                        FontSource::Google,
                        row.family.clone(),
                        row.family,
                        row.category,
                        weights,
                        variable,
                        None,
                        "https://fonts.google.com/".into(),
                    )
                })
                .collect()
        }
        1 => json::<Vec<FontsourceFamily>>(client, &format!("{FONTSOURCE}/fonts"), CATALOGUE_CAP)
            .await?
            .into_iter()
            .filter(|r| r.font_type != "icons")
            .filter_map(|r| {
                entry(
                    FontSource::Fontsource,
                    r.id,
                    r.family,
                    r.category,
                    vec![],
                    None,
                    None,
                    format!(
                        "{} ({})",
                        "https://fontsource.org/",
                        r.license.chars().take(120).collect::<String>()
                    ),
                )
            })
            .collect(),
        2 => json::<ShareCatalogue>(
            client,
            &format!("{FONTSHARE}/fonts?limit=400"),
            CATALOGUE_CAP,
        )
        .await?
        .fonts
        .into_iter()
        .filter_map(|r| {
            let weights = r
                .styles
                .iter()
                .filter(|s| !s.is_italic && s.weight.weight >= 100)
                .map(|s| s.weight.weight)
                .collect();
            let selector = r
                .styles
                .iter()
                .filter(|s| !s.is_italic && s.is_variable)
                .map(|s| s.weight.number.to_string())
                .next();
            entry(
                FontSource::Fontshare,
                r.slug,
                r.name,
                r.category,
                weights,
                None,
                selector,
                match r.license_type.as_str() {
                    "sil_ofl" => "https://fontshare.com/licenses/sil-ofl",
                    _ => "https://fontshare.com/licenses/itf-ffl",
                }
                .into(),
            )
        })
        .collect(),
        _ => unreachable!("provider index"),
    };
    if entries.is_empty() || entries.len() > 5000 {
        return Err(unavailable());
    }
    Ok(entries)
}

fn selected_weights(available: impl IntoIterator<Item = u16>) -> Vec<u16> {
    let mut exact: Vec<_> = available.into_iter().filter(|w| weight(*w)).collect();
    exact.sort_unstable();
    exact.dedup();
    let selected: Vec<_> = exact
        .iter()
        .copied()
        .filter(|n| (400..=700).contains(n))
        .collect();
    if selected.is_empty() {
        exact
            .into_iter()
            .min_by_key(|w| w.abs_diff(400))
            .into_iter()
            .collect()
    } else {
        selected
    }
}

struct Asset {
    url: Url,
    face: Face,
}

fn specification(first: u16, last: u16, italic: bool, unicode_range: Option<String>) -> Face {
    Face {
        digest: String::new(),
        format: Format::Woff2,
        first_weight: first,
        last_weight: last,
        italic,
        unicode_range,
    }
}

fn asset_url(value: &str, host: &str) -> Option<Url> {
    if value.starts_with("//") {
        crate::netguard::checked_url(&format!("https:{value}"), &[host])
    } else {
        crate::netguard::checked_url(value, &[host])
    }
}

fn css_weights(weights: &[u16], delimiter: &str) -> String {
    weights
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(delimiter)
}

async fn google(client: &Client, row: &Entry) -> Result<Vec<Asset>, Error> {
    let mut url = fixed_url(GOOGLE_CSS);
    let weights = selected_weights(row.weights.iter().copied());
    let selector = if let Some((lo, hi)) = row.variable {
        format!("{}:wght@{lo}..{hi}", row.key)
    } else if !weights.is_empty() {
        format!("{}:wght@{}", row.key, css_weights(&weights, ";"))
    } else {
        row.key.clone()
    };
    url.query_pairs_mut()
        .append_pair("family", &selector)
        .append_pair("display", "swap");
    css::parse(&get(client, url, DESCRIPTOR_CAP).await?, GOOGLE_BYTES)
}

async fn fontshare(client: &Client, row: &Entry) -> Result<Vec<Asset>, Error> {
    let mut url = fixed_url(&format!("{FONTSHARE}/css"));
    let weights = selected_weights(row.weights.iter().copied());
    let selector = row
        .variable_selector
        .clone()
        .unwrap_or_else(|| css_weights(&weights, ","));
    if selector.is_empty() {
        return Err(unavailable());
    }
    url.query_pairs_mut()
        .append_pair("f[]", &format!("{}@{selector}", row.key))
        .append_pair("display", "swap");
    css::parse(&get(client, url, DESCRIPTOR_CAP).await?, FONTSHARE_BYTES)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Detail {
    subsets: Vec<String>,
    #[serde(default)]
    unicode_range: BTreeMap<String, String>,
    variants: BTreeMap<u16, BTreeMap<String, BTreeMap<String, Variant>>>,
    #[serde(default)]
    variable: bool,
}
#[derive(Deserialize)]
struct Variant {
    url: AssetLinks,
}
#[derive(Deserialize)]
struct AssetLinks {
    woff2: String,
}
#[derive(Deserialize)]
struct Axes {
    axes: BTreeMap<String, Axis>,
}
#[derive(Deserialize)]
struct Axis {
    min: serde_json::Value,
    max: serde_json::Value,
}
fn axis_limit(v: &serde_json::Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.parse().ok())
}

fn fontsource_range(detail: &Detail, subset: &str) -> Result<Option<String>, Error> {
    detail
        .unicode_range
        .get(subset)
        .map(|value| unicode_range(value).ok_or_else(unavailable))
        .transpose()
}

async fn fontsource(client: &Client, row: &Entry) -> Result<Vec<Asset>, Error> {
    let detail: Detail = json(
        client,
        &format!("{FONTSOURCE}/fonts/{}", row.key),
        DESCRIPTOR_CAP,
    )
    .await?;
    let mut assets = Vec::new();
    if detail.variable {
        let axes: Axes = json(
            client,
            &format!("{FONTSOURCE}/variable/{}", row.key),
            DESCRIPTOR_CAP,
        )
        .await?;
        let (lo, hi) = axes
            .axes
            .get("wght")
            .and_then(|a| range(axis_limit(&a.min)?, axis_limit(&a.max)?))
            .unwrap_or((400, 400));
        let filename_axis = if axes.axes.contains_key("wght") {
            "wght"
        } else {
            "standard"
        };
        let style = if detail.variants.values().any(|s| s.contains_key("normal")) {
            "normal"
        } else {
            "italic"
        };
        let package = format!("@fontsource-variable/{}", row.key);
        for subset in &detail.subsets {
            if !key_valid(subset) {
                return Err(unavailable());
            }
            let url = fixed_url(&format!(
                "https://{FONTSOURCE_BYTES}/npm/{package}/files/{}-{subset}-{filename_axis}-{style}.woff2",
                row.key
            ));
            assets.push(Asset {
                url,
                face: specification(
                    lo,
                    hi,
                    style == "italic",
                    fontsource_range(&detail, subset)?,
                ),
            });
        }
    } else {
        for weight in selected_weights(detail.variants.keys().copied()) {
            let styles = &detail.variants[&weight];
            let (style, sets) = styles
                .get_key_value("normal")
                .or_else(|| styles.get_key_value("italic"))
                .ok_or_else(unavailable)?;
            for (subset, variant) in sets {
                let url = asset_url(&variant.url.woff2, FONTSOURCE_BYTES)
                    .filter(|url| {
                        let prefix = format!("/fontsource/fonts/{}@", row.key);
                        url.path().starts_with(&prefix)
                            && url.path().ends_with(".woff2")
                            && url.query().is_none()
                    })
                    .ok_or_else(unavailable)?;
                assets.push(Asset {
                    url,
                    face: specification(
                        weight,
                        weight,
                        style == "italic",
                        fontsource_range(&detail, subset)?,
                    ),
                });
            }
        }
    }
    if assets.is_empty() || assets.len() > MAX_FACES {
        return Err(unavailable());
    }
    Ok(assets)
}

pub(super) async fn install(client: &Client, entry: Entry) -> Result<RemoteInstall, Error> {
    let assets = match entry.row.source {
        FontSource::Google => google(client, &entry).await?,
        FontSource::Fontsource => fontsource(client, &entry).await?,
        FontSource::Fontshare => fontshare(client, &entry).await?,
        _ => return Err(unavailable()),
    };
    let mut budget = FAMILY_BYTES;
    let mut urls = HashMap::<Url, String>::new();
    let mut faces = Vec::with_capacity(assets.len());
    let mut files = BTreeMap::new();
    for Asset { url, mut face } in assets {
        if let Some(digest) = urls.get(&url) {
            face.digest.clone_from(digest);
        } else {
            let bytes = get(client, url.clone(), budget).await?;
            if Format::identify(&bytes) != Some(Format::Woff2) {
                return Err(unavailable());
            }
            budget -= bytes.len();
            let (digest, bytes) =
                crate::blocking::spawn(move || (crate::store::sha256_hex(&bytes), bytes))
                    .await
                    .map_err(|_| Error::internal("font digest task stopped"))?;
            files.entry(digest.clone()).or_insert(bytes);
            urls.insert(url, digest.clone());
            face.digest = digest;
        }
        faces.push(face);
    }
    Ok(RemoteInstall {
        family: entry.row,
        faces,
        files,
        attribution: entry.attribution,
    })
}
