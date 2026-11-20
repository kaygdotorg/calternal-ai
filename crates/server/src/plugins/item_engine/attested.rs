//! Host-composed attested writes, their checked sources and span reads.
//!
//! The host builds an attested record from package framing and a middle it
//! selects itself: the person's captured composer or editor submission, or
//! the exact validated output of the acting person's attempt. The worker
//! never supplies the middle bytes. A source the invocation cannot prove is
//! refused before any effect; it is never downgraded to an opaque write.
use super::*;

/// Framing bound for each of prefix and suffix, in encoded UTF-8 bytes.
const FRAMING_BYTES: usize = 512;
/// Spans per `item_spans` page (catalogue `ItemSpansPage.spans` maxItems).
const SPAN_PAGE: usize = 128;

/// A person's text captured at checked dispatch from a host-owned composer
/// or editor field, bound to this invocation's actor, operation and request.
/// Only the route that verified the field binding may construct one.
#[derive(Clone, Debug)]
pub struct CapturedInput {
    /// The exact submitted text.
    pub text: String,
    /// The operation's checked target item, or `None` for a create scope.
    pub target: Option<ItemRef>,
}

/// Committed contribution output delivered to the acting person's exact
/// attempt, selected by output sequence.
#[derive(Clone, Debug)]
pub struct AttemptText {
    /// The declared `attempt_output` field of that validated output.
    pub text: String,
    /// Opaque host ref of the exact attempt, recorded as the origin.
    pub attempt: String,
}

/// Resolves an output sequence of the invocation's own attempt. Supplied by
/// the execution runtime for an admitted job; called on the engine worker.
pub type AttemptOutputs = Arc<dyn Fn(u64) -> Result<AttemptText, Error> + Send + Sync>;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Source {
    Input,
    Output { seq: u64 },
    Empty,
}

#[derive(Deserialize)]
pub(super) struct Attested {
    source: Source,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
    entry_anchor: Option<String>,
    quote: Option<Value>,
}

/// One composed record and the evidence bound to it in the same commit.
pub(super) struct Composed {
    pub(super) bytes: Vec<u8>,
    pub(super) middle_offset: usize,
    pub(super) middle_length: usize,
    pub(super) origin: Value,
    pub(super) anchor: Option<String>,
    pub(super) span: String,
    /// Digest of the selected middle, bound into the request fingerprint.
    pub(super) material: String,
}

impl Composed {
    pub(super) fn record(&self) -> calternal_content::SpanRecord<'_> {
        calternal_content::SpanRecord {
            span: &self.span,
            length: self.bytes.len(),
            middle_offset: self.middle_offset,
            middle_length: self.middle_length,
            origin: (self.middle_length > 0).then_some(&self.origin),
            anchor: self.anchor.as_deref(),
        }
    }
}

/// Where the composed record goes, which fixes the admissible shapes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Placing<'a> {
    Create,
    Append(&'a ItemRef),
    Snapshot(&'a ItemRef),
}

impl ItemEngine {
    /// Attach the captured composer or editor text for this invocation. The
    /// caller is the checked activation route that bound the field to the
    /// exact submission; ordinary operation input is never a capture.
    pub fn with_captured_input(mut self, captured: CapturedInput) -> Self {
        self.captured = Some(captured);
        self
    }

    /// Attach the resolver for the invocation's own attempt output.
    pub fn with_attempt_outputs(mut self, outputs: AttemptOutputs) -> Self {
        self.outputs = Some(outputs);
        self
    }

    /// Compose one attested record from checked sources. Cost: one source
    /// lookup and one bounded concatenation; no storage access.
    pub(super) fn compose(
        &self,
        attested: Attested,
        placing: Placing<'_>,
        limit: usize,
    ) -> Result<Composed, Error> {
        if attested.quote.is_some() {
            return Err(Error::unavailable("checked quotes are unavailable"));
        }
        let snapshot = matches!(placing, Placing::Snapshot(_));
        if attested.prefix.len() > FRAMING_BYTES
            || attested.suffix.len() > FRAMING_BYTES
            || (snapshot && !(attested.prefix.is_empty() && attested.suffix.is_empty()))
            || (snapshot && attested.entry_anchor.is_some())
        {
            return Err(Error::invalid_input("invalid attested framing"));
        }
        let origins = &self.operation()["write_origins"];
        let (middle, origin) = match attested.source {
            Source::Input => {
                let captured = self
                    .captured
                    .as_ref()
                    .filter(|_| origins.get("host_input").is_some())
                    .ok_or_else(ineligible)?;
                let target = match placing {
                    Placing::Create => None,
                    Placing::Append(item) | Placing::Snapshot(item) => Some(item),
                };
                if captured.target.as_ref() != target {
                    return Err(ineligible());
                }
                (
                    captured.text.clone(),
                    json!({"kind":"person","actor":self.actor.account}),
                )
            }
            Source::Output { seq } if !snapshot => {
                let outputs = self
                    .outputs
                    .as_ref()
                    .filter(|_| origins.get("attempt_output").is_some())
                    .ok_or_else(ineligible)?;
                let output = outputs(seq)?;
                (
                    output.text,
                    json!({"kind":"attempt","attempt":output.attempt}),
                )
            }
            Source::Output { .. } | Source::Empty => return Err(ineligible()),
        };
        let mut bytes =
            Vec::with_capacity(attested.prefix.len() + middle.len() + attested.suffix.len());
        bytes.extend_from_slice(attested.prefix.as_bytes());
        bytes.extend_from_slice(middle.as_bytes());
        bytes.extend_from_slice(attested.suffix.as_bytes());
        if bytes.is_empty() || bytes.len() > limit {
            return Err(Error::invalid_input(
                "attested record is empty or too large",
            ));
        }
        Ok(Composed {
            middle_offset: attested.prefix.len(),
            middle_length: middle.len(),
            material: crate::store::sha256_hex(middle.as_bytes()),
            bytes,
            origin,
            anchor: attested.entry_anchor,
            span: Uuid::new_v4().to_string(),
        })
    }

    /// Page the recorded spans of a pinned revision under the range rules.
    pub(super) fn spans(&self, request: Spans) -> Result<Value, Error> {
        let item = self.resolve(&request.item)?;
        self.check_range(item, &request.revision)?;
        let after = request
            .cursor
            .as_deref()
            .map(|cursor| self.open_cursor(cursor, item, &request.revision))
            .transpose()?;
        let page = self.content.item_spans(
            self.space,
            self.kind_id(),
            item,
            &request.revision.0,
            after,
            SPAN_PAGE,
        )?;
        self.check_range(item, &request.revision)?;
        let spans: Vec<Value> = page
            .spans
            .into_iter()
            .map(|span| {
                let mut entry = json!({
                    "span": span.span, "offset": span.offset, "length": span.length,
                    "middle_offset": span.middle_offset, "middle_length": span.middle_length,
                });
                if let Some(origin) = span.origin {
                    entry["attribution"] = origin;
                }
                entry
            })
            .collect();
        let mut result = json!({
            "item": request.item, "revision": request.revision,
            "body_provenance": if page.attested { "attested" } else { "unproven" },
            "spans": spans,
        });
        if let Some(after) = page.next {
            result["next"] = json!(self.seal_cursor(item, &request.revision, after));
        }
        Ok(result)
    }

    fn cursor_message(&self, item: Uuid, revision: &RevisionRef, after: u64) -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{after}",
            self.package.id(),
            self.actor.account,
            self.space,
            item,
            revision.0
        )
    }

    fn seal_cursor(&self, item: Uuid, revision: &RevisionRef, after: u64) -> String {
        let tag = ring::hmac::sign(
            cursor_key(),
            self.cursor_message(item, revision, after).as_bytes(),
        );
        let hex: String = tag
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{after}.{hex}")
    }

    /// Cursors are bound to this process, package, actor, item and revision.
    fn open_cursor(&self, cursor: &str, item: Uuid, revision: &RevisionRef) -> Result<u64, Error> {
        let invalid = || Error::invalid_input("invalid span cursor");
        let (after, hex) = cursor.split_once('.').ok_or_else(invalid)?;
        let after: u64 = after.parse().map_err(|_| invalid())?;
        if hex.len() != 64 || !hex.is_ascii() {
            return Err(invalid());
        }
        let tag = (0..32)
            .map(|at| u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| invalid())?;
        let message = self.cursor_message(item, revision, after);
        ring::hmac::verify(cursor_key(), message.as_bytes(), &tag).map_err(|_| invalid())?;
        Ok(after)
    }
}

/// Process-local cursor key. A restart expires every outstanding cursor.
fn cursor_key() -> &'static ring::hmac::Key {
    static KEY: std::sync::OnceLock<ring::hmac::Key> = std::sync::OnceLock::new();
    KEY.get_or_init(|| {
        ring::hmac::Key::generate(ring::hmac::HMAC_SHA256, &ring::rand::SystemRandom::new())
            .expect("system randomness")
    })
}

#[derive(Deserialize)]
pub(super) struct Spans {
    item: ItemRef,
    revision: RevisionRef,
    cursor: Option<String>,
}

fn ineligible() -> Error {
    Error::forbidden("the attested source is not available to this write")
        .with_details(json!({"reason":"ineligible_source"}))
}
