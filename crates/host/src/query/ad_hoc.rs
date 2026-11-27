//! Compile and cache the base declaration for caller-supplied queries.

use crate::declarations::{self, Collection};
use crate::internal;
use calternal_format as format;
use calternal_types::{Error, Result};
use std::sync::OnceLock;

const AD_HOC_SORTS: usize = 8;

pub(super) fn ad_hoc(filter: Option<&str>, sorts: &[String]) -> Result<Collection> {
    if sorts.len() > AD_HOC_SORTS {
        return Err(Error::invalid_input(
            "Use at most eight ad hoc sort columns.",
        ));
    }
    static EMPTY: OnceLock<Result<Collection>> = OnceLock::new();
    // The engine currently exposes a plugin parser as the collection
    // constructor. Compile a dependency-free empty plan once for ad hoc calls.
    let empty = EMPTY.get_or_init(|| {
        let files = [
            format::PluginFile { path:"plugin.md", text:Some("---\nplugin: query-template\nname: Query template\nversion: 1.0.0\nformat: 1\n---\n") },
            format::PluginFile { path:"collections/query.md", text:Some("---\ncollection: query\ntitle: Query\nformat: 1\nfrom: virtual.items\nviews:\n  list: id\n---\n") },
        ];
        format::parse_plugin(&files, &format::InstallOptions::default()).map_err(declarations::invalid)?
            .collections.remove("query").ok_or_else(|| internal("missing ad hoc query template"))
    });
    let mut collection = empty.as_ref().map_err(Clone::clone)?.clone();
    collection.condition = filter
        .map(format::parse_expression)
        .transpose()
        .map_err(declarations::invalid)?;
    collection.sort = sorts
        .iter()
        .map(|column| {
            let (field, descending) = if let Some(field) = column.strip_suffix(" desc") {
                (field, true)
            } else {
                (column.strip_suffix(" asc").unwrap_or(column), false)
            };
            format::Sort {
                field: field.to_owned(),
                descending,
            }
        })
        .collect();
    Ok(collection)
}
