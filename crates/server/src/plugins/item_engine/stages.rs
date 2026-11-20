//! Bounded provisional uploads. Only the writer's final receipt is durable.
use super::*;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

const IDLE: Duration = Duration::from_secs(60);
const CHUNK: usize = 64 * 1024;
const LIMIT: usize = calternal_content::MAX_DOCUMENT_BYTES;

/// Staged-save reservations shared by invocations in one space. The owning
/// space scheduler calls reclaim regularly. A restart drops all provisional
/// bytes, and the committed receipt is replayable from the content writer.
pub struct ItemStages {
    space: SpaceId,
    stages: Mutex<HashMap<String, Stage>>,
}

struct Stage {
    ticket: Uuid,
    actor: AccountId,
    package: String,
    begin: Begin,
    fingerprint: String,
    bytes: Arc<Vec<u8>>,
    sealed: bool,
    refreshed: Instant,
}

impl ItemStages {
    /// Allocate an empty reservation pool for one space. Cost: fixed memory.
    pub fn new(space: SpaceId) -> Self {
        Self {
            space,
            stages: Mutex::default(),
        }
    }

    /// Drop idle uploads without writing storage. Cost: at most eight entries.
    pub fn reclaim(&self) {
        if let Ok(mut stages) = self.stages.lock() {
            Self::expire(&mut stages);
        }
    }

    fn expire(stages: &mut HashMap<String, Stage>) {
        stages.retain(|_, stage| stage.refreshed.elapsed() < IDLE);
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, Stage>>, Error> {
        let mut stages = self
            .stages
            .lock()
            .map_err(|_| Error::unavailable("item staging unavailable"))?;
        Self::expire(&mut stages);
        Ok(stages)
    }

    pub(super) fn retire(&self, ticket: Uuid) {
        if let Ok(mut stages) = self.stages.lock() {
            stages.retain(|_, stage| stage.ticket != ticket);
        }
    }

    pub(super) fn consume(&self, ticket: Uuid, handle: &str) {
        if let Ok(mut stages) = self.stages.lock()
            && stages
                .get(handle)
                .is_some_and(|stage| stage.ticket == ticket)
        {
            stages.remove(handle);
        }
    }

    pub(super) fn begin(
        &self,
        engine: &ItemEngine<'_>,
        input: Begin,
        fp: String,
    ) -> Result<Value, Error> {
        let mut stages = self.lock()?;
        if let Some((handle, existing)) = stages.iter_mut().find(|(_, stage)| {
            stage.actor == engine.actor.account && stage.begin.request_id == input.request_id
        }) {
            if existing.ticket != engine.ticket || existing.fingerprint != fp {
                return Err(Error::request_reused());
            }
            existing.refreshed = Instant::now();
            return Ok(json!({"handle":handle}));
        }
        let mut actor = 0;
        let mut package = 0;
        let mut package_bytes = 0;
        let mut space_bytes = 0;
        for stage in stages.values() {
            actor += usize::from(stage.actor == engine.actor.account);
            if stage.package == engine.package.id() {
                package += 1;
                package_bytes += stage.begin.length;
            }
            space_bytes += stage.begin.length;
        }
        if actor >= 2
            || package >= 4
            || stages.len() >= 8
            || package_bytes + input.length > 8 * 1024 * 1024
            || space_bytes + input.length > 16 * 1024 * 1024
        {
            return Err(Error::new(
                calternal_types::ErrorCode::RetryAfter,
                "item staging limit reached",
            ));
        }
        let handle = Uuid::new_v4().to_string();
        let bytes = Arc::new(Vec::with_capacity(input.length));
        stages.insert(
            handle.clone(),
            Stage {
                ticket: engine.ticket,
                actor: engine.actor.account,
                package: engine.package.id().to_owned(),
                begin: input,
                fingerprint: fp,
                bytes,
                sealed: false,
                refreshed: Instant::now(),
            },
        );
        Ok(json!({"handle":handle}))
    }

    fn get<'a>(
        stages: &'a mut HashMap<String, Stage>,
        engine: &ItemEngine<'_>,
        handle: &str,
    ) -> Result<&'a mut Stage, Error> {
        stages
            .get_mut(handle)
            .filter(|stage| stage.ticket == engine.ticket)
            .ok_or_else(denied)
    }

    fn authorize_stage(&self, engine: &ItemEngine<'_>, handle: &str) -> Result<(), Error> {
        let (item, expected) = {
            let mut stages = self.lock()?;
            let stage = Self::get(&mut stages, engine, handle)?;
            (stage.begin.item.clone(), stage.begin.expected.clone())
        };
        engine.access(
            ItemCheck::Write,
            Some(engine.resolve(&item)?),
            Some(&expected),
        )
    }

    pub(super) fn chunk(&self, engine: &ItemEngine<'_>, input: Chunk) -> Result<Value, Error> {
        self.authorize_stage(engine, &input.handle)?;
        let bytes = body(&input.bytes, CHUNK)?;
        let mut stages = self.lock()?;
        let stage = Self::get(&mut stages, engine, &input.handle)?;
        let end = input
            .offset
            .checked_add(bytes.len())
            .filter(|end| *end <= stage.begin.length)
            .ok_or_else(|| Error::invalid_input("invalid stage offset"))?;
        if bytes.is_empty() || input.offset > stage.bytes.len() {
            return Err(Error::invalid_input("invalid stage offset"));
        }
        let overlap = stage.bytes.len().min(end) - input.offset;
        if stage.bytes[input.offset..input.offset + overlap] != bytes[..overlap] {
            return Err(Error::conflict(
                "staged chunk conflicts with previous bytes",
            ));
        }
        if end > stage.bytes.len() {
            if stage.sealed {
                return Err(Error::conflict("item stage is sealed"));
            }
            Arc::get_mut(&mut stage.bytes)
                .ok_or_else(denied)?
                .extend_from_slice(&bytes[overlap..]);
        }
        stage.refreshed = Instant::now();
        Ok(json!({"next_offset":stage.bytes.len()}))
    }

    pub(super) fn seal(&self, engine: &ItemEngine<'_>, input: Handle) -> Result<Value, Error> {
        self.authorize_stage(engine, &input.handle)?;
        let mut stages = self.lock()?;
        let stage = Self::get(&mut stages, engine, &input.handle)?;
        if !stage.sealed {
            if stage.bytes.len() != stage.begin.length
                || crate::store::sha256_hex(&stage.bytes) != stage.begin.sha256
            {
                return Err(Error::conflict("staged item length or digest differs"));
            }
            stage.sealed = true;
        }
        stage.refreshed = Instant::now();
        Ok(json!({"staged":input.handle}))
    }

    pub(super) fn sealed(
        &self,
        engine: &ItemEngine<'_>,
        input: &Save,
    ) -> Result<(Arc<Vec<u8>>, String), Error> {
        let mut stages = self.lock()?;
        let stage = Self::get(
            &mut stages,
            engine,
            input.staged.as_deref().ok_or_else(denied)?,
        )?;
        if !stage.sealed
            || stage.begin.request_id != input.request_id
            || stage.begin.item != input.item
            || stage.begin.expected != input.expected
        {
            return Err(denied());
        }
        stage.refreshed = Instant::now();
        Ok((stage.bytes.clone(), stage.fingerprint.clone()))
    }
}

impl<'a> ItemEngine<'a> {
    pub(super) fn pool(&self) -> Result<&ItemStages, Error> {
        self.stages
            .ok_or_else(|| Error::unavailable("item staging unavailable"))
    }

    pub(super) fn begin_stage(&self, input: Begin, operand: &Value) -> Result<Value, Error> {
        if input.length == 0 || input.length > LIMIT {
            return Err(Error::invalid_input("invalid staged length"));
        }
        let item = self.resolve(&input.item)?;
        self.access(ItemCheck::Write, Some(item), Some(&input.expected))?;
        let fp = self.fingerprint("item_save_begin", operand)?;
        if let Some(saved) = self.load_receipt(&input.request_id)? {
            if saved.operation != "item_save"
                || saved.item != Some(item)
                || saved.result["stage_begin"] != fp
            {
                return Err(Error::request_reused());
            }
            return self.deliver(item, saved.result["result"].clone());
        }
        // Admission checks the head, then the writer checks it again at commit.
        let (head, _) = self
            .content
            .item_head_reader(self.space)?
            .read_with_size(self.kind_id(), item)?;
        if head.revision != input.expected.0 {
            return Ok(json!({"status":409,"code":"head_conflict","current":self.head(item)?}));
        }
        self.pool()?.begin(self, input, fp)
    }

    /// Attach the space's bounded upload pool to this checked invocation.
    /// A pool from another space is refused. No storage or blocking work.
    pub fn with_stages(mut self, pool: &'a ItemStages) -> Result<Self, Error> {
        if self.space != pool.space {
            return Err(denied());
        }
        self.stages = Some(pool);
        Ok(self)
    }
}

#[derive(Deserialize)]
pub(super) struct Begin {
    request_id: RequestId,
    item: ItemRef,
    expected: RevisionRef,
    length: usize,
    sha256: String,
}

#[derive(Deserialize)]
pub(super) struct Chunk {
    handle: String,
    offset: usize,
    bytes: String,
}

#[derive(Deserialize)]
pub(super) struct Handle {
    handle: String,
}
