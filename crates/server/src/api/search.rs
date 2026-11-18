//! Authorized palette queries and the indexed mention pickers.
use super::*;
use calternal_content::{
    IndexedHit, SearchConfig, SearchKey, SearchKind, SearchMatch, SearchProbe,
};
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
) -> Result<Value, Error> {
    match op {
        SearchOperation::Query => {
            let input: SearchInput = decode(input)?;
            to_value(search(state, person.id, input, false).await?)
        }
        SearchOperation::Items | SearchOperation::Threads => {
            let i: LookupInput = decode(input)?;
            let kinds = if op == SearchOperation::Threads {
                vec!["threads".into()]
            } else {
                Vec::new()
            };
            let input = SearchInput {
                query: i.prefix,
                kinds,
                space_id: i.space_id,
                limit: i.limit,
                cursor: i.cursor,
            };
            to_value(search(state, person.id, input, true).await?)
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
                for plugin in st.registry.plugins() {
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

pub(super) fn configuration(registry: &Registry) -> Result<SearchConfig, Error> {
    SearchConfig::new(
        registry
            .plugins()
            .iter()
            .filter(|p| p.unavailable.is_none())
            .flat_map(|p| {
                p.manifest
                    .kinds
                    .iter()
                    .filter(|k| k.searchable == Some(true))
                    .map(move |k| SearchKind {
                        kind: k.id.clone(),
                        control: crate::store::plugin_key(&p.manifest.id),
                        default_enabled: crate::plugins::default_row(p).enabled,
                        approval: crate::plugins::default_row(p).approval,
                    })
            })
            .collect(),
    )
}

fn directory<D: Directory>(state: &Shared<D>) -> Result<&calternal_auth::spaces::Spaces, Error> {
    state
        .directory
        .protected_spaces()
        .ok_or_else(|| Error::unavailable("search directory unavailable"))
}

async fn search<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    input: SearchInput,
    prefix: bool,
) -> Result<SearchResult, Error> {
    let selected = input.space_id.as_deref().map(space_of).transpose()?;
    let auth = directory(state)?;
    let load = || async {
        auth.search_spaces(account, selected, SearchOperation::CAPABILITY)
            .await
            .map_err(crate::auth::auth_error)
    };
    let spaces = load().await?;
    let selected_spaces = spaces.clone();
    let (input, page) = blocking(state, move |store| {
        let page = collect(store, &selected_spaces, &input, prefix);
        Ok((input, page))
    })
    .await?;
    let current = load().await?;
    if spaces == current {
        return page;
    }
    // Recompute ordering, coverage and continuation if a grant changed.
    retry(state, account, input, prefix, current).await
}

async fn retry<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    input: SearchInput,
    prefix: bool,
    spaces: Vec<SpaceId>,
) -> Result<SearchResult, Error> {
    let filter = input.space_id.as_deref().map(space_of).transpose()?;
    let selected = spaces.clone();
    let result = blocking(state, move |store| {
        collect(store, &selected, &input, prefix)
    })
    .await?;
    let current = directory(state)?
        .search_spaces(account, filter, SearchOperation::CAPABILITY)
        .await
        .map_err(crate::auth::auth_error)?;
    if current != spaces {
        return Err(Error::unavailable("space access changed; retry the search"));
    }
    Ok(result)
}

fn collect(
    store: &Store,
    spaces: &[SpaceId],
    input: &SearchInput,
    prefix: bool,
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
    let after = SearchKey::parse(input.cursor.as_deref())?;
    let kinds = serde_json::to_string(&input.kinds).expect("kind array");
    let folded = calternal_content::fold_title(input.query.trim());
    let limit = input.limit.unwrap_or(20) as usize;
    let mut hits = Vec::new();
    let mut partial = false;
    let probe = |matching: SearchMatch<'_>,
                 hits: &mut Vec<(SpaceId, IndexedHit)>,
                 partial: &mut bool|
     -> Result<(), Error> {
        for &space in spaces {
            let page = store.content().search_page(
                space,
                &SearchProbe {
                    matching,
                    kinds: &kinds,
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
                },
                &mut hits,
                &mut partial,
            )?;
            if after.is_none()
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
    SearchHit {
        address: format!("/{}/{}", hit.kind, hit.id),
        kind: hit.kind,
        space_id: space.to_string(),
        title: hit.title,
        excerpt,
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
