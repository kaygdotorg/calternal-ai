//! The one stopped-installation rewrite from the shipped format to row deltas.
use super::*;

impl Coordinator {
    /// Inspect the checksum-protected snapshot version of an existing recovery
    /// directory. Cost: one snapshot read and decode under the caller's root lock.
    /// Refuses an absent snapshot, unknown version or different installation.
    pub fn stored_version(root: &Path, installation: Uuid) -> Result<u32> {
        if !root.join("snapshot.json").is_file() {
            return Err(invalid());
        }
        let (store, _) = Storage::open(root, Arc::new(|_| Ok(())))?;
        let image: Image = decode(&store.read("snapshot.json")?)?;
        if image.head.installation != installation || !matches!(image.version, LEGACY | CURRENT) {
            return Err(invalid());
        }
        Ok(image.version)
    }

    /// Convert v3 in the stopped installation while the shared runner owns a
    /// durable restore point for the log, database and new `segment` path.
    /// Replays the old chain with its original witness, validates the database,
    /// and compares normalized rows before replacing the snapshot. Costs a
    /// full scan of both stores and a durable snapshot/segment publication.
    /// Failure requires the runner to restore the original files.
    pub fn migrate_from_v3(
        root: &Path,
        installation: Uuid,
        conn: &Connection,
        segment: Uuid,
    ) -> Result<()> {
        if segment.is_nil() || Self::stored_version(root, installation)? != LEGACY {
            return Err(invalid());
        }
        let old = Self::open_version(
            root,
            installation,
            DEFAULT_THRESHOLD,
            Arc::new(|_| Ok(())),
            LEGACY,
        )?;
        let mut cache = old.gate.lock().map_err(err)?;
        let mut s = old.load_cached(&mut cache)?;
        old.replay_loaded(conn, &mut s)?;
        let logical = old.read_loaded(&s, None)?;
        sink::check_authority(conn)?;
        let mut image = s.image;
        image.version = CURRENT;
        let mut records = state::records(conn, installation, CURRENT)?;
        state::normalize_expiry(&mut records, &image.records)?;
        image.records = records;
        image.receipts = receipts::capture(conn)?;
        if image.clone().snapshot(installation)?.tables != logical.tables {
            return Err(err(
                "v3 authority image disagrees with the surviving database",
            ));
        }
        image.segment = format!("log-{segment}.bin");
        old.store.create(&image.segment)?;
        old.store.snapshot(&encode(&image)?)?;
        old.remove_obsolete_segments(&image.segment)?;
        state::reset_capture_triggers(conn)?;
        state::initialize(conn, CURRENT)?;
        Self::open(root, installation)?.validate_against_witness(conn)
    }
}
