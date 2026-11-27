//! Exact DNS names shared by declaration validation and the host egress proxy.

/// Normalize a fully qualified ASCII DNS name in declaration or CONNECT syntax.
/// IP literals, numeric final labels, local names and trailing dots are refused.
/// Resolvers must append the root label before lookup to avoid DNS search.
pub fn outbound_dns_name(host: &str) -> Option<String> {
    let (_, suffix) = host.rsplit_once('.')?;
    if host.len() > 253
        || suffix.is_empty()
        || suffix.bytes().all(|byte| byte.is_ascii_digit())
        || !host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let suffix = host.rsplit_once('.').expect("checked domain").1;
    if matches!(
        suffix,
        "localhost" | "local" | "internal" | "test" | "home" | "lan"
    ) {
        return None;
    }
    Some(host)
}
