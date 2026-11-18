//! Manager edits and package-scoped read access to shared settings.
use calternal_content::{Request, SettingEdit, SettingRecord};
use calternal_core::{
    JsonObject, ManifestSetting, PluginSettingAddress, PluginSettingEdit, PluginSettingValue,
};
use calternal_types::{AccountId, Error, SpaceId};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{Plugin, activation};
use crate::{api::Shared, auth::Directory};

fn input<T: DeserializeOwned>(input: JsonObject) -> Result<T, Error> {
    serde_json::from_value(Value::Object(input.into_iter().collect()))
        .map_err(|_| Error::invalid_input("input does not match setting schema"))
}

pub(super) fn declaration<'a>(plugin: &'a Plugin, key: &str) -> Result<&'a ManifestSetting, Error> {
    plugin
        .manifest
        .settings
        .iter()
        .find(|setting| setting.id == key)
        .ok_or_else(|| Error::not_found("declared plugin setting not found"))
}

pub(super) fn effective(setting: &ManifestSetting, stored: SettingRecord) -> PluginSettingValue {
    let effective = if setting.accepts(&stored.value) {
        stored.value.clone()
    } else {
        setting.default.clone()
    };
    PluginSettingValue {
        effective,
        value: stored.value,
        alternatives: stored.alternatives,
        writable: stored.repair.is_none(),
        repair: stored.repair,
    }
}

fn access<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    space_id: &str,
    plugin_id: &str,
) -> Result<(SpaceId, std::sync::Arc<Plugin>), Error> {
    let space = crate::requests::authorize(
        state,
        account,
        space_id,
        calternal_types::Capability::SpaceSettings,
    )?;
    let catalog = state.registry.snapshot();
    let plugin = catalog
        .get(plugin_id)
        .ok_or_else(|| Error::not_found("installed plugin not found"))?;
    if !activation(&state.store, space, plugin)?.enabled {
        return Err(Error::not_found("enabled plugin not found in this space"));
    }
    Ok((space, plugin.clone()))
}

/// Serve either settings operation on the blocking request pool. A Manager
/// addresses one enabled package and one manifest key. Recheck admission before
/// returning the response. Writes use the existing fingerprint and journal receipts.
pub(crate) fn dispatch<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    rid: &str,
    operation: &str,
    args: JsonObject,
) -> Result<Value, Error> {
    if operation == "plugins.settings.get" {
        let i: PluginSettingAddress = input(args)?;
        let (space, plugin) = access(state, account, &i.space_id, &i.plugin_id)?;
        let setting = declaration(&plugin, &i.key)?;
        let record = state
            .store
            .content()
            .plugin_setting(space, &i.plugin_id, &i.key)?;
        access(state, account, &i.space_id, &i.plugin_id)?;
        return Ok(json!(effective(setting, record)));
    }
    let fp = crate::api::fingerprint(operation, &args);
    let i: PluginSettingEdit = input(args)?;
    let (space, plugin) = access(state, account, &i.space_id, &i.plugin_id)?;
    let setting = declaration(&plugin, &i.key)?;
    if !i.value.is_null() && !setting.accepts(&i.value) {
        return Err(Error::invalid_input(
            "setting value does not match the manifest type",
        ));
    }
    let result = state.store.content().edit_plugin_setting(
        space,
        Request {
            account,
            via: None,
            id: rid,
            operation,
            fingerprint: &fp,
        },
        SettingEdit {
            plugin: &i.plugin_id,
            key: &i.key,
            base: &i.base,
            value: &i.value,
            resolve: i.resolve.as_deref(),
        },
    )?;
    access(state, account, &i.space_id, &i.plugin_id)?;
    Ok(result)
}
