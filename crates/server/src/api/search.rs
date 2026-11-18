//! Authorized palette queries and the indexed mention pickers.
mod shared;
#[cfg(test)]
mod tests;

use super::*;
use calternal_content::{IndexedHit, SearchConfig, SearchKey, SearchMatch, SearchProbe};
use calternal_core::{
    LookupInput, PaletteResult, PeopleLookupResult, SearchHit, SearchInput, SearchOperation,
    SearchResult, SearchSegment, SpaceLookupInput,
};
use calternal_types::Page;

pub(super) async fn dispatch<D: Directory>(
    state: &Shared<D>,
    person: &Identity,
    op: SearchOperation,
    input: JsonObject,
    catalog: &crate::plugins::Catalog,
) -> Result<Value, Error> {
    match op {
        SearchOperation::Query => {
            let input = decode(input)?;
            let plan = QueryPlan::Projected {
                config: catalog.search_configuration().clone(),
                prefix: false,
            };
            to_value(search(state, person.id, input, plan).await?)
        }
        SearchOperation::Items | SearchOperation::Threads => {
            let i: LookupInput = decode(input)?;
            let plan = if op == SearchOperation::Threads {
                QueryPlan::ThreadTitles(
                    catalog
                        .plugins()
                        .iter()
                        .find(|plugin| plugin.declares_kind("threads"))
                        .cloned(),
                )
            } else {
                QueryPlan::Projected {
                    config: catalog.search_configuration().clone(),
                    prefix: true,
                }
            };
            let input = SearchInput {
                query: i.prefix,
                kinds: Vec::new(),
                space_id: i.space_id,
                limit: i.limit,
                cursor: i.cursor,
            };
            to_value(search(state, person.id, input, plan).await?)
        }
        SearchOperation::People => {
            let i: SpaceLookupInput = decode(input)?;
            let people = directory(state)?
                .mentionable(person.id, space_of(&i.space_id)?, i.prefix)
                .await
                .map_err(crate::auth::auth_error)?;
            to_value(PeopleLookupResult { people })
        }
        SearchOperation::Skills => {
            let i: SpaceLookupInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            blocking(state, move |s| {
                super::search_skills::list(s, space, &i.prefix)
            })
            .await
        }
        SearchOperation::Manifests => {
            let i: PluginsListInput = decode(input)?;
            let space = space_of(&i.space_id)?;
            let st = state.clone();
            let manifests = blocking(state, move |store| {
                let mut manifests = Vec::new();
                for plugin in st.registry.snapshot().plugins() {
                    if let Some(menu) = &plugin.palette
                        && activation(store, space, plugin)?.enabled
                    {
                        manifests.push(menu.clone());
                    }
                }
                Ok(manifests)
            })
            .await?;
            to_value(PaletteResult { manifests })
        }
    }
}

fn directory<D: Directory>(state: &Shared<D>) -> Result<&calternal_auth::spaces::Spaces, Error> {
    state
        .directory
        .protected_spaces()
        .ok_or_else(|| Error::unavailable("search directory unavailable"))
}

#[derive(Clone)]
enum QueryPlan {
    Projected {
        config: Arc<SearchConfig>,
        prefix: bool,
    },
    ThreadTitles(Option<Arc<Plugin>>),
}

impl QueryPlan {
    fn collect(
        &self,
        store: &Store,
        spaces: &[SpaceId],
        input: &SearchInput,
        roots: &[shared::Root],
    ) -> Result<SearchResult, Error> {
        match self {
            Self::Projected { config, prefix } => {
                collect(store, spaces, input, *prefix, config, roots)
            }
            Self::ThreadTitles(plugin) => thread_titles(store, spaces, input, plugin.as_deref()),
        }
    }
}

async fn search<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    input: SearchInput,
    plan: QueryPlan,
) -> Result<SearchResult, Error> {
    let selected = input.space_id.as_deref().map(space_of).transpose()?;
    let auth = directory(state)?;
    let load = || async {
        auth.search_spaces(account, selected, SearchOperation::CAPABILITY)
            .await
            .map_err(crate::auth::auth_error)
    };
    let shares = selected.is_none()
        && matches!(plan, QueryPlan::Projected { .. })
        && (input.kinds.is_empty() || input.kinds.iter().any(|kind| kind == "notes"));
    let mut scope = load().await?;
    let mut input = input;
    for _ in 0..2 {
        let roots = if shares {
            auth.search_share_roots(account)
                .await
                .map_err(crate::auth::auth_error)?
        } else {
            Vec::new()
        };
        let authorized = scope.clone();
        let selected_plan = plan.clone();
        let (next_input, page) = blocking(state, move |store| {
            let page = selected_plan.collect(store, &authorized, &input, &roots);
            Ok((input, page))
        })
        .await?;
        let current = load().await?;
        if scope == current {
            let mut page = page?;
            if shares {
                shared::recheck(state, account, &scope, &mut page).await?;
            }
            return Ok(page);
        }
        scope = current;
        input = next_input;
    }
    Err(Error::unavailable("space access changed; retry the search"))
}

// Threads keep title lookup available before message indexing is enabled. Use
// the existing content-head index after checking the declaring plugin's space
// activation. Each enabled space takes one indexed title query and no body reads.
fn thread_titles(
    store: &Store,
    spaces: &[SpaceId],
    input: &SearchInput,
    plugin: Option<&Plugin>,
) -> Result<SearchResult, Error> {
    let prefix = input.query.trim().to_ascii_lowercase();
    let limit = input.limit.unwrap_or(20) as usize;
    let mut hits = Vec::new();
    if let Some(plugin) = plugin {
        for &space in spaces {
            if !activation(store, space, plugin)?.enabled {
                continue;
            }
            let page = store.content().item_page(
                space,
                "threads",
                calternal_content::ItemOrder::Title(&prefix),
                input.cursor.as_deref(),
                limit + 1,
            )?;
            for row in page.rows {
                let title = row.metadata["title"]
                    .as_str()
                    .ok_or_else(|| Error::internal("thread has no indexed title"))?;
                let key = row.metadata["title_key"]
                    .as_str()
                    .map_or_else(|| title.to_ascii_lowercase(), str::to_owned);
                hits.push((
                    (key, row.id),
                    SearchHit {
                        address: format!("/threads/{}", row.id),
                        kind: row.kind,
                        space_id: row.space.to_string(),
                        title: title.to_owned(),
                        excerpt: Vec::new(),
                        message_id: None,
                        message_at: None,
                    },
                ));
            }
            hits.sort_unstable_by(|a, b| a.0.cmp(&b.0));
            hits.truncate(limit + 1);
        }
    }
    let page = Page::from_lookahead(hits, limit, |(key, _)| {
        serde_json::to_string(key).expect("stable title cursor")
    });
    Ok(SearchResult {
        hits: page.rows.into_iter().map(|(_, hit)| hit).collect(),
        next_cursor: page.next,
        partial: false,
    })
}

fn collect(
    store: &Store,
    spaces: &[SpaceId],
    input: &SearchInput,
    prefix: bool,
    config: &SearchConfig,
    roots: &[shared::Root],
) -> Result<SearchResult, Error> {
    let words: Vec<_> = input
        .query
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(12)
        .collect();
    let query = words
        .iter()
        .map(|word| format!("\"{word}\"*"))
        .collect::<Vec<_>>()
        .join(" ");
    let title = format!("title : ({query})");
    let message_terms: Vec<_> = calternal_content::message_terms(&input.query)
        .take(12)
        .collect();
    let raw = input.query.trim();
    let phrase = raw.len() > 1 && raw.starts_with('"') && raw.ends_with('"');
    let messages_only = !input.kinds.is_empty() && input.kinds.iter().all(|kind| kind == "threads");
    let after = SearchKey::parse(input.cursor.as_deref())?;
    let kinds = serde_json::to_string(&input.kinds).expect("kind array");
    let folded = calternal_content::fold_title(input.query.trim());
    let limit = input.limit.unwrap_or(20) as usize;
    let mut hits = Vec::new();
    let (shared_ids, mut partial) = shared::collect_ids(store, spaces, roots)?;
    let probe = |matching: SearchMatch<'_>,
                 hits: &mut Vec<(SpaceId, IndexedHit)>,
                 partial: &mut bool|
     -> Result<(), Error> {
        for (space, item_ids) in spaces.iter().map(|space| (*space, None)).chain(
            shared_ids
                .iter()
                .map(|(space, ids)| (*space, Some(ids.as_str()))),
        ) {
            let page = store.content().search_page(
                config,
                space,
                &SearchProbe {
                    matching,
                    kinds: if item_ids.is_some() {
                        "[\"notes\"]"
                    } else {
                        &kinds
                    },
                    item_ids,
                    after: after.as_ref(),
                    limit: limit + 1,
                },
            )?;
            *partial |= page.partial;
            hits.extend(page.hits.into_iter().map(|hit| (space, hit)));
            hits.sort_unstable_by(|a, b| a.1.key.cmp(&b.1.key));
            hits.truncate(limit + 1);
        }
        Ok(())
    };
    if prefix {
        probe(SearchMatch::Prefix(&folded), &mut hits, &mut partial)?;
    } else if !words.is_empty() {
        let typo_grams = (words.len() == 1)
            .then(|| calternal_content::typo_expression(&folded))
            .flatten();
        if after.as_ref().is_some_and(SearchKey::fuzzy) {
            if let Some(grams) = &typo_grams {
                probe(
                    SearchMatch::Fuzzy {
                        grams,
                        term: &folded,
                    },
                    &mut hits,
                    &mut partial,
                )?;
            }
        } else {
            probe(
                SearchMatch::Text {
                    all: &query,
                    title: &title,
                    message_terms: &message_terms,
                    phrase,
                    messages_only,
                },
                &mut hits,
                &mut partial,
            )?;
            if !messages_only
                && after.is_none()
                && hits.is_empty()
                && let Some(grams) = &typo_grams
            {
                probe(
                    SearchMatch::Fuzzy {
                        grams,
                        term: &folded,
                    },
                    &mut hits,
                    &mut partial,
                )?;
            }
        }
    }
    let page = Page::from_lookahead(hits, limit, |(_, hit)| hit.key.encode());
    Ok(SearchResult {
        hits: page
            .rows
            .into_iter()
            .map(|(space, hit)| wire_hit(space, hit))
            .collect(),
        next_cursor: page.next,
        partial,
    })
}

fn wire_hit(space: SpaceId, hit: IndexedHit) -> SearchHit {
    let mut matched = false;
    let mut excerpt = Vec::new();
    let mut rest = hit.excerpt.as_str();
    let mut left = 240;
    while let Some(at) = rest.find(['\u{1}', '\u{2}']) {
        segment(&mut excerpt, &rest[..at], matched, &mut left);
        matched = rest.as_bytes()[at] == 1;
        rest = &rest[at + 1..];
    }
    segment(&mut excerpt, rest, matched, &mut left);
    let (message_id, message_at) = hit
        .message
        .map_or((None, None), |(id, at)| (Some(id.to_string()), Some(at)));
    SearchHit {
        address: format!("/{}/{}", hit.kind, hit.id),
        kind: hit.kind,
        space_id: space.to_string(),
        title: hit.title,
        excerpt,
        message_id,
        message_at,
    }
}

fn segment(out: &mut Vec<SearchSegment>, text: &str, matched: bool, left: &mut usize) {
    if text.is_empty() || *left == 0 {
        return;
    }
    let mut chars = text.chars();
    let part: String = chars.by_ref().take(*left).collect();
    *left -= part.chars().count();
    out.push(SearchSegment {
        text: part,
        matched,
    });
}
