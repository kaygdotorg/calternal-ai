//! Provider adapter policy for the earlier shared-tool broker.

use calternal_fsio::{beneath::Folder, private};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    io::{self, Read},
    path::Path,
};

/// Fixed provider routes supported by the machine-local inference broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedToolsProvider {
    /// Anthropic Messages API.
    Claude,
    /// OpenAI Responses API.
    Codex,
}

impl SharedToolsProvider {
    /// Filename relative to the approved machine-local credentials folder.
    pub fn credential_name(self) -> &'static str {
        match self {
            Self::Claude => "anthropic.key",
            Self::Codex => "openai.key",
        }
    }

    /// Format a broker-only authorization value. The caller must mark the
    /// resulting header as sensitive and keep it out of logs.
    pub fn format_credential(self, key: &str) -> Cow<'_, str> {
        match self {
            Self::Claude => Cow::Borrowed(key),
            Self::Codex => Cow::Owned(format!("Bearer {key}")),
        }
    }

    /// Return the fixed HTTPS endpoint and credential header for an allowed path.
    pub fn upstream(self, path: &str) -> Option<(&'static str, &'static str)> {
        match (self, path) {
            (Self::Claude, "/v1/messages") => {
                Some(("https://api.anthropic.com/v1/messages", "x-api-key"))
            }
            (Self::Claude, "/v1/messages/count_tokens") => Some((
                "https://api.anthropic.com/v1/messages/count_tokens",
                "x-api-key",
            )),
            (Self::Codex, "/v1/responses") => {
                Some(("https://api.openai.com/v1/responses", "authorization"))
            }
            _ => None,
        }
    }

    /// List the only adapter headers forwarded with an approved inference call.
    pub fn forwarded_headers(self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["anthropic-version", "anthropic-beta"],
            Self::Codex => &[],
        }
    }
}

/// Fully qualified tool name admitted by the workspace broker.
pub const WORKSPACE_TOOL_NAME: &str = "mcp__workspace__workspace_command";
/// Namespace for the workspace command in a nested tool schema.
pub const WORKSPACE_TOOL_NAMESPACE: &str = "mcp__workspace";
/// Local name of the workspace command.
pub const WORKSPACE_TOOL_METHOD: &str = "workspace_command";

/// Copy the request while removing provider-hosted tools. Keeps only the
/// declared workspace command shape for the selected protocol.
pub fn isolate_request(body: &[u8], provider: SharedToolsProvider) -> Option<Vec<u8>> {
    let mut body: serde_json::Value = serde_json::from_slice(body).ok()?;
    let contents = body.as_object_mut()?;
    // Provider-hosted MCP servers bypass the machine's tool executor.
    contents.remove("mcp_servers");
    if let Some(tools) = contents.get_mut("tools") {
        tools.as_array_mut()?.retain_mut(|tool| match provider {
            SharedToolsProvider::Claude => {
                tool["name"] == WORKSPACE_TOOL_NAME
                    && tool.get("type").is_none_or(|kind| kind == "custom")
            }
            SharedToolsProvider::Codex => {
                if tool["type"] == "function" && tool["name"] == WORKSPACE_TOOL_NAME {
                    return true;
                }
                if tool["type"] != "namespace" || tool["name"] != WORKSPACE_TOOL_NAMESPACE {
                    return false;
                }
                tool["tools"].as_array_mut().is_some_and(|functions| {
                    functions.retain(|function| {
                        function["type"] == "function" && function["name"] == WORKSPACE_TOOL_METHOD
                    });
                    !functions.is_empty()
                })
            }
        });
    }
    serde_json::to_vec(&body).ok()
}

const CREDENTIALS: &str = ".credentials.json";
const MAX_CREDENTIAL_BYTES: u64 = 1024 * 1024;

/// Import only the provider's approved login object from a legacy account
/// home. The destination is private and existing credential files win.
pub fn import_legacy_profile(account: &Path, login: &Path) -> io::Result<()> {
    let target = login.join(CREDENTIALS);
    if target.try_exists()? {
        return Ok(());
    }
    let legacy = match Folder::open(account)
        .and_then(|root| root.child("home", false))
        .and_then(|dir| dir.child(".claude", false))
        .and_then(|dir| dir.open_file(CREDENTIALS))
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    legacy
        .take(MAX_CREDENTIAL_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CREDENTIAL_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "provider credential file exceeds limit",
        ));
    }
    let Ok(legacy) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Ok(());
    };
    let Some(claude_login) = legacy
        .get("claudeAiOauth")
        .filter(|value| value.is_object())
    else {
        return Ok(());
    };
    // Older homes may also hold credentials for MCP servers and plugins.
    let bytes = serde_json::to_vec(&serde_json::json!({"claudeAiOauth":claude_login}))?;
    match private::publish(&target, &bytes) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        result => result,
    }
}
