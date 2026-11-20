//! Live authority, publication fences and per-space read handles.
use super::*;

impl Content {
    /// Read one authorized window from the published journal generation.
    /// The callback runs before storage access and before delivery. An
    /// unpublished generation returns loading without rows or a count. A
    /// ready read uses one sidecar/source snapshot, the journal and source
    /// freshness fences, and one head probe per returned row. No flushes.
    pub fn window(
        &self,
        space: SpaceId,
        request: &WindowRequest,
        previous: Option<&WindowResult>,
        authorize: impl Fn() -> Result<WindowAccess>,
    ) -> Result<WindowReply> {
        let access = authorize()?;
        let WindowRequest::Window { request } = request;
        let reauthorize = || {
            if authorize()? == access {
                Ok(())
            } else {
                Err(missing())
            }
        };
        if let Some(reason) = bad_plan(request, &access, previous) {
            reauthorize()?;
            return Ok(reason.into());
        }
        let _step = self.lifecycle.step(space.as_uuid())?;
        let loading = || {
            reauthorize()?;
            self.wake_window(space, true);
            Ok(WindowReply::Loading(LoadingWindow {
                bind: request.bind.clone(),
                state: "loading",
                result_version: None,
                window_ref: None,
                offset: request.from,
                parent: request.parent.clone(),
                rows: [],
                audience: access.audience,
                anchor: request.anchor.unwrap_or_default(),
            }))
        };
        let unavailable = || {
            reauthorize()?;
            self.wake_window(space, true);
            Err(Error::unavailable(
                "window projection is temporarily unavailable",
            ))
        };
        let slot = self.pin_slot(space)?;
        let cache = &slot.windows;
        let source = crate::db::space_path(&self.roots.appdata, space);
        if slot.visibility.get().is_none() && !source.try_exists().map_err(internal)? {
            let reply = evaluate(
                None,
                &access.scope.key(),
                &read::Scope::default(),
                request,
                &access,
                previous,
            )?;
            reauthorize()?;
            return Ok(reply);
        }
        let Ok(pin) = cache.gate.try_read() else {
            return if cache.failure().is_some() {
                unavailable()
            } else {
                loading()
            };
        };
        if cache.initialize_failure(&source).is_err() || cache.failure().is_some() {
            drop(pin);
            return unavailable();
        }
        let opened: Result<Option<runtime::Reader>> = (|| {
            if let Some(reader) = cache.take_reader() {
                return Ok(Some(reader));
            }
            if !replay::path(&source).try_exists().map_err(internal)? {
                return Ok(None);
            }
            let conn = replay::reader(&source)?;
            let Some(journal) =
                calternal_store::ProjectionJournal::open(&self.roots.appdata, space)
                    .map_err(internal)?
            else {
                return Ok(None);
            };
            Ok(Some(runtime::Reader { conn, journal }))
        })();
        let reader = match opened {
            Ok(Some(reader)) => reader,
            Ok(None) => {
                drop(pin);
                return loading();
            }
            Err(error) if error.code == crate::ErrorCode::Conflict => {
                drop(pin);
                return loading();
            }
            Err(_) => {
                cache.report_reader_error();
                drop(pin);
                return unavailable();
            }
        };
        let notes = matches!(&access.scope, WindowScope::SpaceNotes);
        let checked = (|| {
            let conn = &reader.conn;
            let tx = conn.unchecked_transaction().map_err(replay::sql_error)?;
            let Some(publication) = replay::Publication::read(&tx)? else {
                return Ok(None);
            };
            if !publication.fresh_for(&tx, &reader.journal, notes)? {
                return Ok(None);
            }
            let bound: String = tx
                .query_row(
                    "SELECT value FROM source.binding WHERE id='space'",
                    [],
                    |row| row.get(0),
                )
                .map_err(replay::sql_error)?;
            if bound != space.to_string() {
                return Err(missing());
            }
            if let WindowScope::Children { parent, .. } = access.scope
                && !kinds::parent_live(&tx, parent)?
            {
                return Err(missing());
            }
            let scope = access.scope.key();
            let snapshot = read::scope(&tx, &scope)?;
            let reply = evaluate(Some(&tx), &scope, &snapshot, request, &access, previous)?;
            tx.commit().map_err(replay::sql_error)?;
            Ok(Some((reply, publication)))
        })();
        let result = match checked {
            Ok(Some((reply, publication))) => {
                reauthorize()?;
                // Source before journal: a preceding write is visible either
                // as a pending outbox entry or a durable journal advance.
                match publication.fresh_for(&reader.conn, &reader.journal, notes) {
                    Ok(true) => Ok(Some(reply)),
                    Ok(false) => Ok(None),
                    Err(error) => Err(error),
                }
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        if result.is_ok() {
            cache.put_reader(reader, &self.window_handles);
        }
        drop(pin);
        match result {
            Ok(Some(reply)) => {
                self.lifecycle.check(space.as_uuid())?;
                Ok(reply)
            }
            Ok(None) => loading(),
            Err(error) if error.code == crate::ErrorCode::NotFound => Err(error),
            Err(error) if error.code == crate::ErrorCode::Conflict => loading(),
            Err(_) => {
                cache.report_reader_error();
                unavailable()
            }
        }
    }
}
