//! Authority-barrier attachment and publication using the existing coordinator cache.
use super::*;
impl RecoverySink for Coordinator {
    fn serial(&self) -> &Mutex<()> {
        &self.serial
    }
    fn reconcile(&self, c: &Connection) -> calternal_auth::Result<()> {
        let _revision = calternal_types::begin_item_access_change();
        let run = || -> Result<()> {
            if !c.is_autocommit() {
                return Err(invalid());
            }
            #[cfg(feature = "perf-timings")]
            let start = std::time::Instant::now();
            let mut cache = self.gate.lock().map_err(err)?;
            let mut s = self.load_cached(&mut cache)?;
            #[cfg(feature = "perf-timings")]
            let loaded = start.elapsed();
            let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='security_recovery_commits')",[],|r|r.get(0)).map_err(err)?;
            if !exists && (s.image.head.sequence > 0 || s.pending.is_some() || !s.tail.is_empty()) {
                return Err(invalid());
            }
            state::initialize(c)?;
            self.replay_loaded(c, &mut s)?;
            self.compact(&mut s, false)?;
            // Only the snapshot-selected segment participates in validation.
            self.remove_obsolete_segments(&s.image.segment)?;
            let repair = !s.compared;
            if s.image.committed_sequence > 0 && !repair {
                // Reuse the verified current map, but retain the existing cache's
                // replacement detection before exposing authority. The host owns
                // trusted ancestry/root locks, as on every cached prepare call.
                let stamps = self.stamps(&s.image.segment)?;
                if stamps != s.stamps {
                    return Err(invalid());
                }
                self.read_loaded(&s, None)?;
            }
            *cache = Some(s);
            drop(cache);
            if repair {
                #[cfg(feature = "perf-timings")]
                let reconciled = start.elapsed();
                self.compare_and_repair(c)?;
                #[cfg(feature = "perf-timings")]
                if startup_timings_enabled() {
                    let compared = start.elapsed() - reconciled;
                    eprintln!("STARTUP authority_recovery_cache_us {}", loaded.as_micros());
                    eprintln!(
                        "STARTUP authority_witness_maintenance_us {}",
                        (reconciled - loaded).as_micros()
                    );
                    eprintln!("STARTUP authority_compare_us {}", compared.as_micros());
                }
            }
            Ok(())
        };
        run().map_err(auth_error)
    }
    fn compact_erasure(&self, c: &Connection) -> calternal_auth::Result<()> {
        let run = || -> Result<()> {
            let mut cache = self.gate.lock().map_err(err)?;
            let mut s = self.load_cached(&mut cache)?;
            self.replay_loaded(c, &mut s)?;
            self.compact(&mut s, true)?;
            self.remove_obsolete_segments(&s.image.segment)?;
            *cache = Some(s);
            Ok(())
        };
        run().map_err(auth_error)
    }
    fn prepare(&self, c: &Connection) -> calternal_auth::Result<String> {
        self.prepare_inner(c).map_err(auth_error)
    }
    fn complete(&self, c: &Connection, id: &str) -> calternal_auth::Result<()> {
        let run = || -> Result<()> {
            if !c.is_autocommit() {
                return Err(invalid());
            }
            if id.is_empty() {
                return Ok(());
            }
            let id = Uuid::parse_str(id).map_err(err)?;
            let mut cache = self.gate.lock().map_err(err)?;
            let mut s = self.load_cached(&mut cache)?;
            self.store.tick(Step::AfterCommit)?;
            self.replay_loaded(c, &mut s)?;
            if s.image.transaction != id {
                return Err(err("requested transaction did not commit"));
            }
            self.compact(&mut s, false)?;
            *cache = Some(s);
            Ok(())
        };
        run().map_err(auth_error)
    }
    fn compact_retired_receipts(&self, c: &Connection) -> calternal_auth::Result<()> {
        self.replay(c).map_err(auth_error)
    }
}
impl Coordinator {
    pub(super) fn remove_obsolete_segments(&self, active: &str) -> Result<()> {
        let mut removed = false;
        for name in self.store.names()? {
            let snapshot_temp =
                name.starts_with(calternal_fsio::TEMP_PREFIX) && name.ends_with("-snapshot.json");
            if (Self::segment_name(&name) && name != active) || snapshot_temp {
                self.store.remove(&name)?;
                removed = true;
            }
        }
        if removed {
            self.store.sync()?;
        }
        Ok(())
    }

    // The caller holds authority admission or the authority publication guard.
    // Compare only after the witness proved the original database and image.
    fn compare_and_repair(&self, conn: &Connection) -> Result<()> {
        let tx =
            rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
        let mut records = state::records(&tx, self.installation)?;
        let receipts = receipts::capture(&tx)?;
        let changed = {
            let cache = self.gate.lock().map_err(err)?;
            let image = &cache.as_ref().ok_or_else(invalid)?.image;
            state::normalize_expiry(&mut records, &image.records)?;
            let mut changed = !state::dirty(&tx)?.is_empty();
            for key in records.keys().chain(image.records.keys()) {
                if records.get(key) != image.records.get(key) {
                    tx.execute(
                        "INSERT OR IGNORE INTO security_recovery_dirty VALUES(?1)",
                        [key],
                    )
                    .map_err(err)?;
                    changed = true;
                }
            }
            for key in receipts.0.keys().chain(image.receipts.0.keys()) {
                if receipts.0.get(key) != image.receipts.0.get(key) {
                    receipts::mark(&tx, key)?;
                    changed = true;
                }
            }
            if changed {
                check_authority(&tx)?;
            }
            changed
        };
        let id = if changed {
            self.prepare_inner(&tx)?
        } else {
            String::new()
        };
        tx.commit().map_err(err)?;
        self.complete(conn, &id).map_err(err)?;
        let mut cache = self.gate.lock().map_err(err)?;
        let state = cache.as_mut().ok_or_else(invalid)?;
        self.read_loaded(state, None)?;
        state.compared = true;
        Ok(())
    }

    /// Recapture protected records after a locked schema upgrade, before the
    /// deployment admits writers. The deployment intent covers both the SQL
    /// migration and these recovery writes. Cost scales with current authority.
    pub fn refresh_after_migration(&self, conn: &Connection) -> Result<()> {
        state::reset_capture_triggers(conn)?;
        self.reconcile(conn).map_err(err)?;
        let _serial = self.serial.lock().map_err(err)?;
        let tx =
            rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
        state::mark_all(&tx)?;
        let intent = self.prepare(&tx).map_err(err)?;
        tx.commit().map_err(err)?;
        self.complete(conn, &intent).map_err(err)
    }
}
pub(super) fn check_authority(conn: &Connection) -> Result<()> {
    let status: String = conn
        .query_row("PRAGMA integrity_check(1)", [], |row| row.get(0))
        .map_err(err)?;
    let mut foreign_keys = conn.prepare("PRAGMA foreign_key_check").map_err(err)?;
    if status != "ok" || foreign_keys.exists([]).map_err(err)? {
        return Err(invalid());
    }
    Ok(())
}
fn auth_error(e: impl std::fmt::Display) -> calternal_auth::Error {
    calternal_auth::Error::Internal(e.to_string())
}
