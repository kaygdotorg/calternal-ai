//! Provider command preparation and startup preflight for a fenced job.
use super::{AgentInput, Authority, Directory, Error, Job, Shared, Uuid, blocking};
use calternal_core::{PROVIDER_COMMAND_BYTES, ProviderCommand, ProviderRequest};

/// A missing executable is a definite setup failure, before any provider effect.
/// Constant script and one executable basename in argv; credentials remain in env.
pub(super) fn argv(executable: String) -> Vec<String> {
    vec!["sh".into(),"-c".into(),r#"
if ! command -v "$1" >/dev/null 2>&1; then
    printf '%s\n' '{"version":1,"seq":1,"event":{"type":"error","code":"adapter_missing","class":"permanent","billing":"unknown","message":"Provider adapter is missing; install it in this machine sandbox (for a source checkout, run npm run build:plugins)."}}' '{"version":1,"seq":2,"event":{"type":"done","outcome":"failed"}}'
    exit 127
fi
exec "$1"
"#.into(),"calternal-provider-launch".into(),executable]
}

/// Bind the active job's existing agent scope before asking the machine to run.
/// Reads the job and writes one protected scope; no token is issued here.
pub(super) async fn prepare_authority<D: Directory>(
    state: &Shared<D>,
    job: &Job,
    i: &AgentInput,
    thread: Uuid,
) -> Result<Authority, Error> {
    let (s, sp, id, a) = (
        state.clone(),
        job.space.clone(),
        job.id.clone(),
        Authority {
            actor: calternal_types::Actor::agent(job.account, thread),
            space: job.space.clone(),
            job: job.id.clone(),
            mode: i.mode,
            source_app: i.source_app.clone(),
            grants: vec![],
        },
    );
    let issuing_run = job.runs;
    blocking(move || {
        let scope = if s.jobs.get(&sp, &id)?.is_some_and(|j| j.runs > 1) {
            s.jobs.agent_scope(&sp, &id)?
        } else {
            a
        };
        s.jobs.agent_prepare_scope(&scope, issuing_run)
    })
    .await
}

/// Encode the bounded newline-delimited provider commands. No file I/O.
pub(super) fn encode(commands: Vec<ProviderCommand>) -> Result<String, Error> {
    let mut stdin = String::new();
    for (seq, command) in commands.into_iter().enumerate() {
        let line = serde_json::to_string(&ProviderRequest {
            version: 1,
            seq: seq as u64 + 1,
            command,
        })
        .map_err(|_| Error::internal("provider encoding"))?;
        if line.len() + 1 > PROVIDER_COMMAND_BYTES {
            return Err(Error::invalid_input(
                "provider instructions exceed command budget",
            ));
        }
        stdin.push_str(&line);
        stdin.push('\n');
    }
    Ok(stdin)
}
