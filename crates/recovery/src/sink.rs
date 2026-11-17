//! Authority-barrier attachment and publication using the existing coordinator cache.
use super::*;
impl RecoverySink for Coordinator {
    fn serial(&self) -> &Mutex<()> {
        &self.serial
    }
    fn reconcile(&self, c: &Connection) -> calternal_auth::Result<()> {
        let run = || -> Result<()> {
            if !c.is_autocommit() {
                return Err(invalid());
            }
            let mut cache = self.gate.lock().map_err(err)?;
            let mut s = self.load_cached(&mut cache)?;
            let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='security_recovery_commits')",[],|r|r.get(0)).map_err(err)?;
            if !exists && (s.image.head.sequence > 0 || s.pending.is_some() || !s.tail.is_empty()) {
                return Err(invalid());
            }
            state::initialize(c)?;
            self.replay_loaded(c, &mut s)?;
            self.compact(&mut s)?;
            // Obsolete segments and fsio temporary files are harmless; only the
            // exact snapshot-selected segment participates in validation.
            let mut removed = false;
            for name in self.store.names()? {
                if Self::segment_name(&name) && name != s.image.segment {
                    self.store.remove(&name)?;
                    removed = true;
                }
            }
            if removed {
                self.store.sync()?;
            }
            if s.image.committed_sequence > 0 {
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
            self.compact(&mut s)?;
            *cache = Some(s);
            Ok(())
        };
        run().map_err(auth_error)
    }
}
fn auth_error(e: impl std::fmt::Display) -> calternal_auth::Error {
    calternal_auth::Error::Internal(e.to_string())
}
