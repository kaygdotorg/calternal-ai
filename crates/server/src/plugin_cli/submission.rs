//! Offline gallery envelopes that retain the existing installable package format.
use super::{CliError, Reply, check, local, package_error};
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_fsio::beneath::Folder;
use calternal_server::plugins::package_store::Package;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::Path,
};

const FORMAT: &str = "calternal-plugin-submission/v1";
const MAX_LISTING: usize = 32 * 1024;
const MAX_SCREENSHOT: usize = 2 * 1024 * 1024;
const MAX_ENVELOPE: usize = 16 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Listing {
    summary: String,
    description: String,
    author: String,
    source: String,
    license: String,
    permissions: String,
    screenshots: Vec<Screenshot>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Screenshot {
    file: String,
    caption: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Submission {
    format: String,
    package: Package,
    package_sha256: String,
    listing: Listing,
    screenshots: Vec<String>,
}

fn invalid(message: impl Into<String>) -> CliError {
    CliError::new("invalid_input", message)
        .hint("Use the submission guide to check the listing, screenshots and package.")
}

fn read(mut input: impl Read, limit: usize) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    input
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(local)?;
    if bytes.len() > limit {
        return Err(invalid("Submission input exceeds its size limit."));
    }
    Ok(bytes)
}

fn decode<T: DeserializeOwned>(bytes: &[u8], name: &str) -> Result<T, CliError> {
    serde_json::from_slice(bytes).map_err(|error| invalid(format!("{name}: {error}")))
}

fn text(value: &str, name: &str, max: usize) -> Result<(), CliError> {
    if value.trim().is_empty() || value.len() > max {
        return Err(invalid(format!(
            "Listing {name} must contain 1 to {max} bytes of text."
        )));
    }
    Ok(())
}

impl Listing {
    fn validate(&self) -> Result<(), CliError> {
        for (name, value, max) in [
            ("summary", self.summary.as_str(), 160),
            ("description", self.description.as_str(), 8_192),
            ("author", self.author.as_str(), 128),
            ("source", self.source.as_str(), 1_024),
            ("license", self.license.as_str(), 128),
            ("permissions", self.permissions.as_str(), 8_192),
        ] {
            text(value, name, max)?;
        }
        let source = reqwest::Url::parse(&self.source)
            .map_err(|_| invalid("Listing source must be an HTTPS repository URL."))?;
        if source.scheme() != "https"
            || source.host_str().is_none()
            || !source.username().is_empty()
            || source.password().is_some()
            || source.query().is_some()
            || source.fragment().is_some()
        {
            return Err(invalid(
                "Listing source must be an HTTPS repository URL without credentials, a query or a fragment.",
            ));
        }
        if !(1..=3).contains(&self.screenshots.len()) {
            return Err(invalid("List one to three PNG screenshots."));
        }
        let mut names = BTreeSet::new();
        for shot in &self.screenshots {
            text(&shot.caption, "screenshot caption", 256)?;
            let stem = shot.file.strip_suffix(".png").unwrap_or("");
            if stem.is_empty()
                || stem.len() > 64
                || !stem
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
                || !names.insert(&shot.file)
            {
                return Err(invalid("Screenshot files need unique plain .png names."));
            }
        }
        Ok(())
    }
}

fn screenshot(bytes: &[u8]) -> Result<(), CliError> {
    // Check the format header and decoded dimensions without decoding image data.
    let dimensions = bytes.get(16..24).filter(|_| {
        bytes.starts_with(b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR")
            && bytes.ends_with(b"\x00\x00\x00\x00IEND\xae\x42\x60\x82")
    });
    let valid = dimensions.is_some_and(|dimensions| {
        dimensions
            .chunks_exact(4)
            .all(|n| (1..=4096).contains(&u32::from_be_bytes(n.try_into().unwrap())))
    });
    if !valid || bytes.len() > MAX_SCREENSHOT {
        return Err(invalid(
            "Screenshots must be PNG files, at most 2 MiB and 4096 pixels per side.",
        ));
    }
    Ok(())
}

impl Submission {
    fn validate(&self) -> Result<(), CliError> {
        if self.format != FORMAT {
            return Err(invalid("Unknown submission format."));
        }
        self.package.validate().map_err(package_error)?;
        if self.package.checksum().map_err(package_error)? != self.package_sha256 {
            return Err(invalid("The embedded package checksum does not match."));
        }
        self.listing.validate()?;
        if self.listing.screenshots.len() != self.screenshots.len() {
            return Err(invalid("The screenshot data does not match the listing."));
        }
        for image in &self.screenshots {
            screenshot(
                &STANDARD
                    .decode(image)
                    .map_err(|_| invalid("Invalid screenshot encoding."))?,
            )?;
        }
        Ok(())
    }

    fn reply(&self, archive: &Path) -> Reply {
        let m = &self.package.manifest;
        Reply::new(
            json!({ "archive": archive, "format": FORMAT, "id": m.id, "version": m.version,
            "package_sha256": self.package_sha256, "screenshots": self.screenshots.len() }),
        )
        .fields([
            ("Submission", archive.display().to_string()),
            ("Plugin", m.id.clone()),
            ("Version", m.version.clone()),
            ("Package SHA-256", self.package_sha256.clone()),
            ("Screenshots", self.screenshots.len().to_string()),
        ])
    }
}

pub(super) fn create(source: &Path, archive: &Path) -> Result<Reply, CliError> {
    check(source)?;
    let package = Package::read(source).map_err(package_error)?;
    if source.join("worker.ts").is_file() && package.worker.is_none() {
        return Err(invalid(
            "The worker bundle is missing. Run npm run build first.",
        ));
    }
    let folder = Folder::open_existing(source).map_err(local)?;
    let listing: Listing = decode(
        &read(
            folder.open_file("gallery.json").map_err(local)?,
            MAX_LISTING,
        )?,
        "gallery.json",
    )?;
    listing.validate()?;
    let mut screenshots = Vec::with_capacity(listing.screenshots.len());
    for shot in &listing.screenshots {
        let path = format!("screenshots/{}", shot.file);
        let bytes = read(folder.open_file(&path).map_err(local)?, MAX_SCREENSHOT)?;
        screenshot(&bytes)?;
        screenshots.push(STANDARD.encode(bytes));
    }
    let submission = Submission {
        format: FORMAT.into(),
        package_sha256: package.checksum().map_err(package_error)?,
        package,
        listing,
        screenshots,
    };
    let bytes = serde_json::to_vec(&submission).map_err(|e| invalid(e.to_string()))?;
    if bytes.len() > MAX_ENVELOPE {
        return Err(invalid("The submission exceeds 16 MiB."));
    }
    let (dir, name) = archive_location(archive)?;
    let mut temp = dir
        .create_temp(&format!(
            "{}{}",
            calternal_fsio::TEMP_PREFIX,
            uuid::Uuid::new_v4()
        ))
        .map_err(local)?;
    temp.file().write_all(&bytes).map_err(local)?;
    temp.persist(|attempt| (attempt == 1).then(|| name.to_owned()))
        .map_err(local)?;
    Ok(submission.reply(archive).hint(
        "Run calternal plugin verify-package on this archive before submitting it for review.",
    ))
}

fn archive_location(archive: &Path) -> Result<(Folder, &str), CliError> {
    let name = archive
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("Choose a submission archive filename."))?;
    let parent = archive
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let dir = Folder::open_existing(parent).map_err(local)?;
    Ok((dir, name))
}

pub(super) fn verify(archive: &Path) -> Result<Reply, CliError> {
    let (dir, name) = archive_location(archive)?;
    let bytes = read(dir.open_file(name).map_err(local)?, MAX_ENVELOPE)?;
    let submission: Submission = decode(&bytes, "Submission archive")?;
    submission.validate()?;
    Ok(submission
        .reply(archive)
        .line("Submission archive passed offline validation."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn source(dir: &Path) {
        let manifest = include_str!("../../../../packages/plugin-sdk/templates/manifest.json")
            .replace("__PLUGIN__", "submission-fixture");
        fs::write(dir.join("manifest.json"), manifest).unwrap();
        fs::write(
            dir.join("README.md"),
            "# Fixture\n\nDocs for submission-fixture.hello.",
        )
        .unwrap();
        fs::create_dir(dir.join("screenshots")).unwrap();
        let image = STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aO2QAAAAASUVORK5CYII=").unwrap();
        fs::write(dir.join("screenshots/view.png"), image).unwrap();
        fs::write(
            dir.join("gallery.json"),
            json!({
                "summary":"A fixture", "description":"One generated view", "author":"Example",
                "source":"https://example.com/fixture", "license":"MIT",
                "permissions":"Reads one stored item when requested.",
                "screenshots":[{"file":"view.png","caption":"Default view"}],
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn submission_round_trips_without_overwriting_and_detects_package_changes() {
        let dir = tempfile::tempdir().unwrap();
        source(dir.path());
        let archive = dir.path().join("plugin.calsubmission");
        let other = dir.path().join("other.calsubmission");
        assert!(create(dir.path(), &archive).is_ok());
        assert!(verify(&archive).is_ok());
        assert!(create(dir.path(), &other).is_ok());
        let bytes = fs::read(&archive).unwrap();
        assert_eq!(bytes, fs::read(&other).unwrap());
        assert!(create(dir.path(), &archive).is_err());
        assert_eq!(bytes, fs::read(&archive).unwrap());
        let mut changed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        changed["package"]["manifest"]["name"] = json!("Different plugin");
        fs::write(&archive, changed.to_string()).unwrap();
        assert!(verify(&archive).is_err());
    }

    #[test]
    fn submission_keeps_unbuilt_workers_and_unsafe_screenshots_out() {
        let dir = tempfile::tempdir().unwrap();
        source(dir.path());
        let archive = dir.path().join("plugin.calsubmission");
        fs::write(dir.path().join("worker.ts"), "export {};").unwrap();
        assert!(create(dir.path(), &archive).is_err());
        fs::remove_file(dir.path().join("worker.ts")).unwrap();
        let listing = dir.path().join("gallery.json");
        let mut input: serde_json::Value =
            serde_json::from_slice(&fs::read(&listing).unwrap()).unwrap();
        input["screenshots"][0]["file"] = json!("../view.png");
        fs::write(listing, input.to_string()).unwrap();
        assert!(create(dir.path(), &archive).is_err());
        assert!(!archive.exists());
    }
}
