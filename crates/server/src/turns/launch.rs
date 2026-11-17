//! Provider startup preflight inside the selected sandbox, without reading stderr.
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
