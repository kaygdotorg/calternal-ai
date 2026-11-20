//! Exact-revision read and disclosure boundaries.
use super::*;
use std::time::Duration;

impl ItemEngine<'_> {
    /// Retain a checked revision furnished by another host route, such as
    /// a view lease. The live range authority check still applies at use.
    /// Costs one bounded map entry for at most 60 seconds.
    pub fn pin_revision(&mut self, item: &ItemRef, revision: RevisionRef) -> Result<(), Error> {
        let item = self.resolve(item)?;
        self.access(ItemCheck::Range, Some(item), Some(&revision))?;
        self.retain_pin(item, revision)
    }

    pub(super) fn retain_pin(&mut self, item: Uuid, revision: RevisionRef) -> Result<(), Error> {
        let now = Instant::now();
        self.ranges.retain(|_, deadline| *deadline > now);
        let key = (item, revision);
        if self.ranges.len() == calternal_plugin_host::MAX_CALLBACKS
            && !self.ranges.contains_key(&key)
        {
            return Err(Error::unavailable("item range pin limit reached"));
        }
        self.ranges.insert(key, now + Duration::from_secs(60));
        Ok(())
    }

    /// Recheck one pinned source immediately before disclosing a body or
    /// derivative. The caller invokes this on its delivery worker after
    /// dequeue and with the same invocation authority used for the read.
    pub fn authorize_range_delivery(
        &self,
        item: &ItemRef,
        revision: &RevisionRef,
    ) -> Result<(), Error> {
        self.check_range(self.resolve(item)?, revision)
    }

    pub(super) fn check_range(&self, item: Uuid, revision: &RevisionRef) -> Result<(), Error> {
        if self
            .ranges
            .get(&(item, revision.clone()))
            .is_none_or(|until| *until <= Instant::now())
        {
            return Err(denied());
        }
        self.access(ItemCheck::Range, Some(item), Some(revision))
    }

    pub(super) fn head(&self, item: Uuid) -> Result<Value, Error> {
        self.access(ItemCheck::Head, Some(item), None)?;
        let (mut head, bytes) = self
            .content
            .item_head_reader(self.space)?
            .read_with_size(self.kind_id(), item)?;
        let revision = RevisionRef(head.revision);
        self.access(ItemCheck::Head, Some(item), Some(&revision))?;
        let fields = head.metadata.as_object_mut().ok_or_else(denied)?;
        let mut permitted = serde_json::Map::new();
        for (key, value) in std::mem::take(fields) {
            let access = ItemAccess {
                check: ItemCheck::Metadata,
                item: Some(item),
                revision: Some(revision.clone()),
                field: Some(key.clone()),
            };
            match (self.authorize)(access) {
                Ok(()) => {
                    permitted.insert(key, value);
                }
                Err(error)
                    if matches!(
                        error.code,
                        calternal_types::ErrorCode::Forbidden
                            | calternal_types::ErrorCode::NotFound
                    ) => {}
                Err(error) => return Err(error),
            }
        }
        let reference = self.content.checked_item_ref(self.space, item)?;
        self.access(ItemCheck::Head, Some(item), Some(&revision))?;
        Ok(json!({
            "item":reference, "revision":revision, "bytes":bytes,
            "locator":locator(self.space, item), "metadata":permitted
        }))
    }
}
