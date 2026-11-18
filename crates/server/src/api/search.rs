//! Authorized palette queries and the indexed mention pickers.
use super::*;
use calternal_core::{PaletteResult, PeopleLookupResult, SearchOperation, SpaceLookupInput};
pub(super) async fn dispatch<D: Directory>(
    state: &Shared<D>,
    person: &Identity,
    op: SearchOperation,
    input: JsonObject,
) -> Result<Value, Error> {
    match op {
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
        .ok_or_else(|| Error::unavailable("mention directory unavailable"))
}
