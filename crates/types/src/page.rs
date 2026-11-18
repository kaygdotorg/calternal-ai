//! Keyset page assembly shared by the directory and content indexes.

/// A bounded page of rows and its continuation position, before transport encoding.
pub struct Page<T, K = String> {
    /// Ordered rows selected from one read snapshot.
    pub rows: Vec<T>,
    /// Key of the last delivered row if the query found another row.
    pub next: Option<K>,
}

impl<T, K> Page<T, K> {
    /// Trim the indexed query's extra row and derive the continuation from the
    /// last delivered row. Cost is one truncation and at most one key extraction.
    pub fn from_lookahead(mut rows: Vec<T>, limit: usize, key: impl FnOnce(&T) -> K) -> Self {
        let more = rows.len() > limit;
        rows.truncate(limit);
        let next = if more { rows.last().map(key) } else { None };
        Self { rows, next }
    }
}
