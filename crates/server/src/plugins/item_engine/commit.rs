//! The checked commit path: request fingerprint, durable receipt replay,
//! one content-writer commit and the delivery recheck.
use super::*;

impl ItemEngine {
    pub(super) fn write(&self, write: Write<'_>) -> Result<Value, Error> {
        let check = if write.target.is_some() {
            ItemCheck::Write
        } else {
            ItemCheck::Create
        };
        let guard = || self.access(check, write.target, write.expected);
        guard()?;
        if self.kind().get("extractors").is_some() {
            return Err(Error::unavailable("item extractors are unavailable"));
        }
        let material = write.record.map(|record| record.material.as_str());
        let fingerprint = self.fingerprint(write.method, write.operand, material)?;
        let key = receipt_key(&write.id);
        let request = Request {
            account: self.actor.account,
            via: self.actor.via.as_ref(),
            id: &key,
            operation: write.method,
            fingerprint: &fingerprint,
        };
        let encode = |item: &Item| {
            let receipt = ReceiptRef(write.id.0.clone());
            let revision = RevisionRef(item.revision.clone());
            let mut result = if write.method == "item_append" {
                json!({"revision":revision,"end_offset":item.bytes.len(),"receipt":receipt})
            } else {
                json!({"item":write.reference, "revision":revision, "locator":locator(self.space,item.id),"receipt":receipt})
            };
            if let Some(record) = write.record {
                result["span"] = json!(record.span);
            }
            match write.stage_begin {
                None => result,
                Some(begin) => json!({"stage_begin":begin,"result":result}),
            }
        };
        let replay = || -> Result<Option<(Uuid, Value)>, Error> {
            let Some(saved) = self.load_receipt(&write.id)? else {
                return Ok(None);
            };
            guard()?;
            self.replay(saved, write.method, &fingerprint, write.target)
                .map(Some)
        };
        let (item, result) = if let Some(replay) = replay()? {
            replay
        } else {
            let discovery = if self.kind()["attempt_discovery"] == "item_policy" {
                // Raw plugin bytes set or retain consent_only (catalogue
                // authorship case import_or_unproven_write).
                if write.opaque {
                    DiscoveryWrite::Protect
                } else {
                    DiscoveryWrite::Checked {
                        allow_creator_only: write.marker == Some("creator_only"),
                        protect: write.marker == Some("consent_only"),
                    }
                }
            } else {
                if write.marker.is_some() {
                    return Err(denied());
                }
                DiscoveryWrite::Unmanaged
            };
            match self.content.write_checked_item(
                self.space,
                self.kind_id(),
                write.target,
                write.expected,
                write.placement,
                write.metadata,
                write.body,
                discovery,
                write.record.map(Composed::record),
                self.fence
                    .as_ref()
                    .map(|(key, value)| ControlFence { key, value }),
                request,
                guard,
                |value| {
                    self.package
                        .check_item_metadata(self.kind_id(), value)
                        .map_err(|_| Error::invalid_input("invalid item metadata"))
                },
                encode,
            ) {
                Ok(item) => (item.id, wire_result(encode(&item))),
                Err(error) => replay()?.ok_or(error)?,
            }
        };
        self.deliver(item, result)
    }

    /// Request fingerprint over the package build, operation, invocation,
    /// method and operand, plus the digest of a host-selected attested middle
    /// so an equal operand naming a different capture conflicts.
    pub(super) fn fingerprint(
        &self,
        method: &str,
        operand: &Value,
        material: Option<&str>,
    ) -> Result<String, Error> {
        let bytes = serde_json::to_vec(&(
            self.package.id(),
            &self.package.declaration()["worker"]["sha256"],
            &self.operation()["id"],
            &self.invocation,
            method,
            operand,
            material,
        ))
        .map_err(|_| Error::invalid_input("invalid item operand"))?;
        Ok(crate::store::sha256_hex(&bytes))
    }

    pub(super) fn load_receipt(
        &self,
        id: &RequestId,
    ) -> Result<Option<calternal_content::Receipt>, Error> {
        self.content
            .receipt(self.space, self.actor.account, &receipt_key(id))
    }

    pub(super) fn replay(
        &self,
        saved: calternal_content::Receipt,
        method: &str,
        fingerprint: &str,
        target: Option<Uuid>,
    ) -> Result<(Uuid, Value), Error> {
        if saved.operation != method || saved.fingerprint != fingerprint {
            return Err(Error::request_reused());
        }
        let item = saved.item.ok_or_else(denied)?;
        if target.is_some_and(|target| target != item) {
            return Err(denied());
        }
        Ok((item, wire_result(saved.result)))
    }

    pub(super) fn deliver(&self, item: Uuid, result: Value) -> Result<Value, Error> {
        let revision = decode::<RevisionRef>(&result["revision"])?;
        self.access(ItemCheck::Deliver, Some(item), Some(&revision))?;
        Ok(result)
    }
}

pub(super) fn receipt_key(id: &RequestId) -> String {
    format!("plugin/v2/{}", crate::store::sha256_hex(id.0.as_bytes()))
}

pub(super) fn wire_result(mut result: Value) -> Value {
    if result.get("stage_begin").is_some() {
        result["result"].take()
    } else {
        result
    }
}
