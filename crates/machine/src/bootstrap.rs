//! Resume the co-located deployment handoff without sending a network code.
use std::{io, path::Path, time::Duration};

use crate::identity::{Credentials, MachineKey};
use crate::protocol::{LocalEnrollment, Owner};
use crate::{Error, Result};

pub(crate) async fn wait(dir: &Path, handoff: &Path, server: &str) -> Result<()> {
    let root = dir.to_owned();
    tokio::task::spawn_blocking(move || calternal_fsio::private::directory(&root))
        .await
        .map_err(|_| Error::Protocol("could not prepare the machine directory".into()))??;
    let _ownership = crate::ownership::lock(dir).await?;
    let (dir, handoff, server) = (dir.to_owned(), handoff.to_owned(), server.to_owned());
    loop {
        let (dir, handoff, server) = (dir.clone(), handoff.clone(), server.clone());
        if tokio::task::spawn_blocking(move || advance(&dir, &handoff, &server))
            .await
            .map_err(|_| Error::Protocol("local enrollment task failed".into()))??
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn advance(dir: &Path, handoff: &Path, server: &str) -> Result<bool> {
    let pending = dir.join("pending-enrollment");
    let request = handoff.join("request.json");
    let confirmation = handoff.join("enrolled");
    if Credentials::path(dir).try_exists()? {
        if Credentials::path(&pending).try_exists()? {
            remove(&request)?;
            remove(&confirmation)?;
            remove(&Credentials::path(&pending))?;
            calternal_fsio::sync_dir(handoff)?;
            calternal_fsio::sync_dir(&pending)?;
        }
        return Ok(true);
    }
    let creds = if Credentials::path(&pending).try_exists()? {
        Credentials::load(&pending)?
    } else {
        let creds = Credentials {
            server: crate::link::Link::new(server)?.base().to_owned(),
            machine_id: uuid::Uuid::new_v4(),
            owner: Owner::Pool,
            key: MachineKey::generate()?,
        };
        creds.save(&pending)?;
        creds
    };
    if confirmation.try_exists()? {
        if std::fs::read_to_string(&confirmation)?.trim() != creds.machine_id.to_string() {
            return Err(Error::Protocol(
                "local enrollment confirmation did not match".into(),
            ));
        }
        creds.save(dir)?;
        remove(&request)?;
        remove(&confirmation)?;
        remove(&Credentials::path(&pending))?;
        calternal_fsio::sync_dir(handoff)?;
        calternal_fsio::sync_dir(&pending)?;
        return Ok(true);
    }
    if !request.try_exists()? {
        let payload = serde_json::to_vec(&LocalEnrollment {
            machine_id: creds.machine_id,
            public_key: creds.key.public_key_b64(),
            name: crate::enroll::host_name().chars().take(64).collect(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
        })
        .map_err(|_| Error::Protocol("could not encode local enrollment".into()))?;
        calternal_fsio::private::publish(&request, &payload)?;
    }
    Ok(false)
}

fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
