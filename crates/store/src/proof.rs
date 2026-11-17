//! Small, bounded caches for signed sync tokens and verified cursor proofs.

/// The largest token either bounded cursor cache retains.
const MAX_TOKEN_BYTES: usize = 64;

/// The number of recent signed event tokens retained for catch-up and replies.
pub(crate) const MAX_ISSUED_TOKENS: usize = 128;

/// A fixed-size-in-practice cache for the last exact token already checked.
#[derive(Default)]
pub(crate) struct ProofCache {
    /// One signed cursor and its sequence; no token over the fixed byte cap is kept.
    proof: Option<CachedProof>,
}

/// The token bytes and sequence shared by cache lookups.
struct CachedProof {
    /// Exact token text, capped by [`MAX_TOKEN_BYTES`].
    token: String,
    /// Journal position proven by that token.
    seq: i64,
}

impl ProofCache {
    /// Replaces the cached token when it fits the fixed budget.
    pub(crate) fn remember(&mut self, token: &str, seq: i64) {
        if token.len() <= MAX_TOKEN_BYTES {
            self.proof = Some(CachedProof {
                token: token.to_owned(),
                seq,
            });
        }
    }

    /// Returns the sequence only when every byte matches the cached token.
    pub(crate) fn lookup(&self, token: &str) -> Option<i64> {
        self.proof
            .as_ref()
            .filter(|proof| proof.token == token)
            .map(|proof| proof.seq)
    }

    /// Forgets the prior epoch's proof before a new epoch is published.
    pub(crate) fn clear(&mut self) {
        self.proof = None;
    }
}

/// Keeps a fixed recent window so repeated reads can reuse already signed tokens.
pub(crate) struct IssuedTokenCache {
    /// The one epoch represented by the slots, checked once for each page.
    epoch: Option<String>,
    /// Direct-mapped slots; page reads preserve hits while caching their misses.
    entries: Vec<Option<IssuedToken>>,
}

/// One token retained for a journal sequence in the cache's current epoch.
struct IssuedToken {
    /// The journal position that was signed.
    seq: i64,
    /// The exact token returned to a client.
    token: String,
}

impl Default for IssuedTokenCache {
    /// Creates exactly the fixed number of empty token slots.
    fn default() -> Self {
        Self {
            epoch: None,
            entries: (0..MAX_ISSUED_TOKENS).map(|_| None).collect(),
        }
    }
}

impl IssuedTokenCache {
    /// Clears old slots before a new epoch reuses journal sequence numbers.
    fn ensure_epoch(&mut self, epoch: &str) {
        if self.epoch.as_deref() != Some(epoch) {
            self.clear();
            self.epoch = Some(epoch.to_owned());
        }
    }

    /// Keeps a short token unless a newer sequence already owns its slot.
    pub(crate) fn remember(&mut self, epoch: &str, seq: i64, token: &str) {
        if seq < 0 || token.len() > MAX_TOKEN_BYTES {
            return;
        }
        self.ensure_epoch(epoch);
        let slot = seq.rem_euclid(MAX_ISSUED_TOKENS as i64) as usize;
        if self.entries[slot]
            .as_ref()
            .is_some_and(|existing| existing.seq > seq)
        {
            return;
        }
        self.entries[slot] = Some(IssuedToken {
            seq,
            token: token.to_owned(),
        });
    }

    /// Finds the token only when both its epoch and sequence match exactly.
    pub(crate) fn lookup(&self, epoch: &str, seq: i64) -> Option<&str> {
        if seq < 0 || self.epoch.as_deref() != Some(epoch) {
            return None;
        }
        let slot = seq.rem_euclid(MAX_ISSUED_TOKENS as i64) as usize;
        self.entries[slot]
            .as_ref()
            .filter(|entry| entry.seq == seq)
            .map(|entry| entry.token.as_str())
    }

    /// Looks up one page under one cache lock and reports whether it has misses.
    pub(crate) fn lookup_page(
        &self,
        epoch: &str,
        sequences: impl Iterator<Item = i64>,
    ) -> (Vec<Option<String>>, bool) {
        if self.epoch.as_deref() != Some(epoch) {
            return (sequences.map(|_| None).collect(), true);
        }
        let mut has_misses = false;
        let page = sequences
            .map(|seq| {
                if seq < 0 {
                    has_misses = true;
                    return None;
                }
                let slot = seq.rem_euclid(MAX_ISSUED_TOKENS as i64) as usize;
                let token = self.entries[slot]
                    .as_ref()
                    .filter(|entry| entry.seq == seq)
                    .map(|entry| entry.token.clone());
                has_misses |= token.is_none();
                token
            })
            .collect();
        (page, has_misses)
    }

    /// Caches a page's missing tokens while preserving the tokens it reused.
    ///
    /// The bounded slots let an old page reuse its signatures after one read,
    /// while a page larger than the cache keeps its existing hits.
    pub(crate) fn remember_page(
        &mut self,
        epoch: &str,
        tokens: &[(i64, String)],
        protected: impl Iterator<Item = i64>,
    ) {
        self.ensure_epoch(epoch);
        let mut protected_slots = [false; MAX_ISSUED_TOKENS];
        for seq in protected.filter(|seq| *seq >= 0) {
            let slot = seq.rem_euclid(MAX_ISSUED_TOKENS as i64) as usize;
            protected_slots[slot] = true;
        }
        for (seq, token) in tokens {
            if *seq < 0 || token.len() > MAX_TOKEN_BYTES {
                continue;
            }
            let slot = seq.rem_euclid(MAX_ISSUED_TOKENS as i64) as usize;
            if protected_slots[slot] {
                continue;
            }
            self.entries[slot] = Some(IssuedToken {
                seq: *seq,
                token: token.clone(),
            });
        }
    }

    /// Drops every cached cursor when a new journal epoch is published.
    pub(crate) fn clear(&mut self) {
        self.epoch = None;
        for entry in &mut self.entries {
            *entry = None;
        }
    }

    /// Counts occupied slots for the bounded-cache regression test.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.iter().filter(|entry| entry.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    //! Checks exact proof matching and the fixed bound on recently issued cursors.

    use super::{IssuedTokenCache, MAX_ISSUED_TOKENS, ProofCache};

    /// Only the exact short cursor is reused, and epoch changes can clear it.
    #[test]
    fn proof_cache_matches_exact_tokens_and_can_be_cleared() {
        let mut cache = ProofCache::default();
        let token = "epoch-7-00000001-00000002";

        cache.remember(token, 7);

        assert_eq!(cache.lookup(token), Some(7));
        assert_eq!(cache.lookup("epoch-7-00000001-00000003"), None);
        cache.clear();
        assert_eq!(cache.lookup(token), None);
    }

    /// Cursor text longer than the fixed cache budget is never retained.
    #[test]
    fn proof_cache_does_not_retain_long_tokens() {
        let mut cache = ProofCache::default();
        let token = "x".repeat(65);

        cache.remember(&token, 7);

        assert_eq!(cache.lookup(&token), None);
    }

    /// A new epoch drops old slots before its sequence numbers are reused.
    #[test]
    fn issued_token_cache_discards_old_epoch_before_sequence_reuse() {
        let mut cache = IssuedTokenCache::default();
        cache.remember("epoch-a", 1, "epoch-a-1");
        cache.remember("epoch-a", 129, "epoch-a-129");

        cache.remember("epoch-b", 1, "epoch-b-1");

        assert_eq!(cache.lookup("epoch-a", 1), None);
        assert_eq!(cache.lookup("epoch-a", 129), None);
        assert_eq!(cache.lookup("epoch-b", 1), Some("epoch-b-1"));
    }

    /// Reports whether a page read found signatures that still need caching.
    #[test]
    fn page_lookup_reports_cache_misses() {
        let mut cache = IssuedTokenCache::default();
        let (missing, needs_cache) = cache.lookup_page("epoch-a", [1, 2].into_iter());

        assert!(needs_cache);
        assert_eq!(missing, vec![None, None]);

        cache.remember_page(
            "epoch-a",
            &[(1, "epoch-a-1".to_owned()), (2, "epoch-a-2".to_owned())],
            std::iter::empty(),
        );
        let (cached, needs_cache) = cache.lookup_page("epoch-a", [1, 2].into_iter());

        assert!(!needs_cache);
        assert_eq!(
            cached,
            vec![Some("epoch-a-1".to_owned()), Some("epoch-a-2".to_owned())]
        );

        let (partial, needs_cache) = cache.lookup_page("epoch-a", [1, 3].into_iter());

        assert!(needs_cache);
        assert_eq!(partial, vec![Some("epoch-a-1".to_owned()), None]);
    }

    /// A page fill caches its older cursor so the next read can reuse it.
    #[test]
    fn older_page_fill_reuses_the_recently_requested_cursor() {
        let mut cache = IssuedTokenCache::default();
        cache.remember("epoch-a", 129, "epoch-a-129");

        cache.remember_page(
            "epoch-a",
            &[(1, "epoch-a-1".to_owned())],
            std::iter::empty(),
        );

        assert_eq!(cache.lookup("epoch-a", 1), Some("epoch-a-1"));
        assert_eq!(cache.lookup("epoch-a", 129), None);
    }

    /// The cache retains at most the newest token in each fixed sequence slot.
    #[test]
    fn issued_token_cache_is_bounded_and_prefers_newer_sequences() {
        let mut cache = IssuedTokenCache::default();
        for seq in 1..=(MAX_ISSUED_TOKENS as i64 + 1) {
            cache.remember("epoch-a", seq, &format!("epoch-a-{seq}-check-tag"));
        }

        assert_eq!(cache.len(), MAX_ISSUED_TOKENS);
        assert_eq!(cache.lookup("epoch-a", 1), None);
        assert_eq!(cache.lookup("epoch-a", 2), Some("epoch-a-2-check-tag"));
        assert_eq!(
            cache.lookup("epoch-a", MAX_ISSUED_TOKENS as i64 + 1),
            Some("epoch-a-129-check-tag")
        );
        assert_eq!(cache.lookup("epoch-b", 2), None);

        cache.clear();
        assert_eq!(cache.len(), 0);
    }
}
