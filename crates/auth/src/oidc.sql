CREATE TABLE oidc_providers (
    id TEXT PRIMARY KEY,
    issuer TEXT NOT NULL UNIQUE,
    client_id TEXT NOT NULL,
    config TEXT NOT NULL,
    secret BLOB NOT NULL,
    revision TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1))
) STRICT;

CREATE TABLE oidc_links (
    provider_id TEXT NOT NULL REFERENCES oidc_providers(id) ON DELETE CASCADE,
    subject TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY(provider_id, subject),
    UNIQUE(provider_id, user_id)
) STRICT;
CREATE INDEX oidc_links_user ON oidc_links(user_id);

CREATE TABLE oidc_sessions (
    installation_id TEXT PRIMARY KEY REFERENCES installations(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL REFERENCES oidc_providers(id) ON DELETE CASCADE,
    subject TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE
) STRICT;
CREATE INDEX oidc_sessions_provider ON oidc_sessions(provider_id);
CREATE INDEX oidc_sessions_user ON oidc_sessions(user_id);
