-- Senders' pictures, shared by every user, as square JPEGs. `key` is `gravatar:<SHA-256 of the
-- address>` or `company:<domain>`; `image` is null when there is none. Each is fetched again
-- after a while.
CREATE TABLE avatars (
    key TEXT PRIMARY KEY,
    image BYTEA,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
