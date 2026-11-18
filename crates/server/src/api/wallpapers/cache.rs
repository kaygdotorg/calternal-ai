//! Bounded appdata cache. Every file belongs to a fixed catalog slot. Startup
//! evicts obsolete slots and abandoned staging files left by interrupted writes.

use super::{catalog, upstream};
use calternal_types::Error;
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

pub(super) struct Image {
    pub bytes: Vec<u8>,
    pub etag: String,
}

impl Image {
    fn new(bytes: Vec<u8>) -> Self {
        let etag = format!("\"{}\"", crate::store::sha256_hex(&bytes));
        Self { bytes, etag }
    }
}

pub(super) async fn open(appdata: &Path) -> Result<PathBuf, Error> {
    let dir = appdata.join("wallpapers");
    io_job(move || {
        calternal_fsio::private::directory(&dir)?;
        for item in fs::read_dir(&dir)? {
            let item = item?;
            let metadata = item.path().symlink_metadata()?;
            let name = item.file_name();
            let cap = max_bytes(&name.to_string_lossy());
            if metadata.is_file() && cap != 0 && metadata.len() <= cap as u64 {
                continue;
            }
            if !metadata.is_dir() {
                fs::remove_file(item.path())?;
            }
        }
        Ok(dir)
    })
    .await
}

fn max_bytes(name: &str) -> usize {
    if let Some((id, rendition)) = name.split_once('.') {
        if rendition == "tracked" && catalog::PHOTOS.iter().any(|p| p.0 == id) {
            return TRACKED.len();
        }
        if let Some((_, r)) = catalog::find(id, rendition) {
            return r.max_bytes;
        }
    }
    0
}

pub(super) async fn read(path: PathBuf, cap: usize) -> Result<Option<Image>, Error> {
    io_job(move || {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut bytes = Vec::new();
        file.take(cap as u64 + 1).read_to_end(&mut bytes)?;
        Ok((bytes.len() <= cap && upstream::jpeg(&bytes)).then(|| Image::new(bytes)))
    })
    .await
}

pub(super) async fn put(path: PathBuf, bytes: Vec<u8>) -> Result<Image, Error> {
    io_job(move || {
        calternal_fsio::write_atomic(&path, &bytes)?;
        Ok(Image::new(bytes))
    })
    .await
}

const TRACKED: &[u8] = b"tracked: true\n";

pub(super) async fn tracked(path: PathBuf) -> Result<bool, Error> {
    io_job(move || match File::open(path) {
        Ok(file) => {
            let mut marker = Vec::new();
            file.take(TRACKED.len() as u64 + 1)
                .read_to_end(&mut marker)?;
            Ok(marker == TRACKED)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    })
    .await
}

pub(super) async fn mark_tracked(path: PathBuf) -> Result<(), Error> {
    io_job(move || calternal_fsio::write_atomic(&path, TRACKED)).await
}

async fn io_job<T: Send + 'static>(
    f: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> Result<T, Error> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| Error::internal("wallpaper cache task stopped"))?
        .map_err(|_| Error::internal("wallpaper cache could not be accessed"))
}
