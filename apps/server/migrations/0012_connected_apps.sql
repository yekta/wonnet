-- Applications a user lets reach their mail over MCP, through OAuth 2.1. A client registers itself
-- here (RFC 7591), or is described by a metadata document whose URL is its client_id, which isn't
-- kept.
CREATE TABLE oauth_clients (
    client_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    client_uri TEXT NOT NULL DEFAULT '',
    redirect_uris TEXT[] NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- What the user approved for a client, until they revoke it. `access` is `crate::access::Access`.
CREATE TABLE oauth_grants (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL,
    client_name TEXT NOT NULL,
    client_kind TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    access JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX oauth_grants_by_user ON oauth_grants(user_id) WHERE revoked_at IS NULL;

-- A consent the client hasn't exchanged yet. Exchanging it makes `grant_id`; a code exchanged
-- twice revokes that grant.
CREATE TABLE oauth_codes (
    code_hash TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL,
    client_name TEXT NOT NULL,
    client_kind TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    access JSONB NOT NULL,
    grant_id UUID REFERENCES oauth_grants(id) ON DELETE CASCADE,
    used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL
);

-- A grant's tokens, `access` or `refresh`. A refresh token is used once.
CREATE TABLE oauth_tokens (
    token_hash TEXT PRIMARY KEY,
    grant_id UUID NOT NULL REFERENCES oauth_grants(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ
);

CREATE INDEX oauth_tokens_by_grant ON oauth_tokens(grant_id);

-- A browser signed in to approve a connected app or to manage them. `csrf` goes in its forms.
CREATE TABLE web_sessions (
    token_hash TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    csrf TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);

-- A sign-in started on one of those pages goes back to it instead of to the app.
ALTER TABLE sign_ins ADD COLUMN web_next TEXT;

CREATE INDEX messages_by_thread ON messages(account_id, thread_id) WHERE NOT deleted;
CREATE INDEX messages_by_sender ON messages(user_id, from_email) WHERE NOT deleted;
