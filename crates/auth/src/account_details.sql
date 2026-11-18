CREATE TABLE account_details (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    email TEXT,
    verified_at INTEGER,
    prompt_dismissed INTEGER NOT NULL DEFAULT 0 CHECK (prompt_dismissed IN (0, 1)),
    replacement_pending INTEGER NOT NULL DEFAULT 0 CHECK (replacement_pending IN (0, 1)),
    CHECK (verified_at IS NULL OR email IS NOT NULL)
) STRICT;

CREATE UNIQUE INDEX account_details_verified_email
    ON account_details(email) WHERE verified_at IS NOT NULL;

CREATE TABLE account_recovery_codes (
    hash BLOB PRIMARY KEY CHECK (length(hash) = 32),
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX account_recovery_codes_user ON account_recovery_codes(user_id);

CREATE TABLE account_email_challenges (
    id TEXT PRIMARY KEY,
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    binding BLOB NOT NULL,
    email TEXT NOT NULL,
    proof BLOB NOT NULL CHECK (length(proof) = 32),
    expires_at INTEGER NOT NULL,
    attempts INTEGER NOT NULL CHECK (attempts BETWEEN 0 AND 5),
    confirmed INTEGER NOT NULL DEFAULT 0 CHECK (confirmed IN (0, 1))
) STRICT;
CREATE INDEX account_email_challenges_expiry ON account_email_challenges(expires_at);
CREATE INDEX account_email_challenges_user ON account_email_challenges(user_id);
CREATE INDEX account_email_challenges_binding ON account_email_challenges(binding);

CREATE TABLE account_recovery_flows (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    stage TEXT NOT NULL CHECK (stage IN ('proof','waiting','claim')),
    proof BLOB NOT NULL CHECK (length(proof) = 32),
    expires_at INTEGER NOT NULL,
    hold_until INTEGER,
    proof_until INTEGER NOT NULL,
    attempts INTEGER NOT NULL CHECK (attempts BETWEEN 0 AND 5)
) STRICT;
CREATE UNIQUE INDEX account_recovery_flow_user ON account_recovery_flows(user_id);
CREATE INDEX account_recovery_flow_expiry ON account_recovery_flows(expires_at);

CREATE TABLE account_recovery_grants (
    hash BLOB PRIMARY KEY CHECK (length(hash) = 32),
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    method TEXT NOT NULL CHECK(method IN ('code','email','admin')),
    expires_at INTEGER NOT NULL
) STRICT;
CREATE UNIQUE INDEX account_recovery_grant_user ON account_recovery_grants(user_id);
CREATE INDEX account_recovery_grant_expiry ON account_recovery_grants(expires_at);

CREATE TABLE account_security_events (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    method TEXT NOT NULL,
    action TEXT NOT NULL,
    actor TEXT NOT NULL,
    at INTEGER NOT NULL
) STRICT;
CREATE INDEX account_security_events_user ON account_security_events(user_id,at);
CREATE INDEX account_security_events_expiry ON account_security_events(at);
