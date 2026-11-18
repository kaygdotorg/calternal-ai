//! Manager edits and package-scoped read access to shared settings.
use calternal_content::{Request, SettingEdit, SettingRecord};
use calternal_core::{
    JsonObject, ManifestSetting, ManifestSettingScope, PluginPersonalSettingEdit,
    PluginSettingAddress, PluginSettingEdit, PluginSettingValue, RequestOperation,
};
use calternal_types::{AccountId, Capability, Error, SpaceId};
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
    required: Capability,
) -> Result<(SpaceId, std::sync::Arc<Plugin>), Error> {
    let space = crate::requests::authorize(state, account, space_id, required)?;
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
        let (space, plugin) = access(
            state,
            account,
            &i.space_id,
            &i.plugin_id,
            Capability::SpaceSettings,
        )?;
        let setting = scoped(&plugin, &i.key, false)?;
        let record = state
            .store
            .content()
            .plugin_setting(space, &i.plugin_id, &i.key)?;
        access(
            state,
            account,
            &i.space_id,
            &i.plugin_id,
            Capability::SpaceSettings,
        )?;
        return Ok(json!(effective(setting, record)));
    }
    let fp = crate::api::fingerprint(operation, &args);
    let i: PluginSettingEdit = input(args)?;
    let (space, plugin) = access(
        state,
        account,
        &i.space_id,
        &i.plugin_id,
        Capability::SpaceSettings,
    )?;
    let setting = scoped(&plugin, &i.key, false)?;
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
    access(
        state,
        account,
        &i.space_id,
        &i.plugin_id,
        Capability::SpaceSettings,
    )?;
    Ok(result)
}

fn scoped<'a>(plugin: &'a Plugin, key: &str, personal: bool) -> Result<&'a ManifestSetting, Error> {
    let setting = declaration(plugin, key)?;
    if (setting.scope == Some(ManifestSettingScope::Personal)) != personal {
        return Err(Error::not_found("declared plugin setting not found"));
    }
    Ok(setting)
}

/// Serve the account-local settings operations on the blocking request pool.
/// The authenticated account owns the override and its invalidation receipt.
pub(crate) fn personal<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    rid: &str,
    row: &RequestOperation,
    args: JsonObject,
) -> Result<Value, Error> {
    if row.id == "plugins.personal_settings.get" {
        let i: PluginSettingAddress = input(args)?;
        let (space, plugin) = access(state, account, &i.space_id, &i.plugin_id, row.required)?;
        let setting = scoped(&plugin, &i.key, true)?;
        let value = state
            .inputs
            .plugin_value(account, space, &i.plugin_id, &i.key)?;
        access(state, account, &i.space_id, &i.plugin_id, row.required)?;
        return Ok(json!(effective(
            setting,
            SettingRecord {
                value,
                alternatives: Vec::new(),
                repair: None
            }
        )));
    }
    let fp = crate::api::fingerprint(row.id, &args);
    let i: PluginPersonalSettingEdit = input(args)?;
    let (space, plugin) = access(state, account, &i.space_id, &i.plugin_id, row.required)?;
    let setting = scoped(&plugin, &i.key, true)?;
    if !i.value.is_null() && !setting.accepts(&i.value) {
        return Err(Error::invalid_input(
            "setting value does not match the manifest type",
        ));
    }
    let lifecycle = state.store.content().lifecycle();
    let _space = lifecycle.step(space.as_uuid())?;
    let reply = state.inputs.edit_plugin_value(
        account,
        crate::account_input::PersonalEdit {
            space,
            plugin: &i.plugin_id,
            key: &i.key,
            base: &i.base,
            value: &i.value,
            default: &setting.default,
            request: rid,
            fingerprint: &fp,
            retention: state.store.content().receipt_retention(),
        },
    )?;
    access(state, account, &i.space_id, &i.plugin_id, row.required)?;
    Ok(json!(reply))
}
