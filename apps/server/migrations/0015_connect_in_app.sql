-- Applications are approved in the apps now, not on a page a browser signs in to.
DROP TABLE web_sessions;
ALTER TABLE sign_ins DROP COLUMN web_next;

-- A browser sent to /oauth/authorize, waiting for the user to answer in the app. `key_hash` is
-- the secret of the link that opens the app on it; `browser_hash` the cookie of the browser that
-- asked, the only one the answer goes back to. `access` is `crate::access::Access`.
CREATE TABLE oauth_requests (
    id UUID PRIMARY KEY,
    key_hash TEXT NOT NULL UNIQUE,
    browser_hash TEXT NOT NULL,
    client_id TEXT NOT NULL,
    client_name TEXT NOT NULL,
    client_kind TEXT NOT NULL,
    client_verified BOOLEAN NOT NULL,
    client_publisher TEXT,
    redirect_uri TEXT NOT NULL,
    state TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    access JSONB,
    approved BOOLEAN,
    answered_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL
);
