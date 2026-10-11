-- The devices that get push notifications. `token` is the APNs device token, `topic` the app's
-- bundle id, `sandbox` true for a development build. `key` (sealed with SECRET_KEY) encrypts
-- what a notification says, so only the device reads it.
CREATE TABLE devices (
    token TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    topic TEXT NOT NULL,
    sandbox BOOLEAN NOT NULL,
    key BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX devices_by_user ON devices(user_id);

-- The messages a notification was pushed for, until they are read or leave the inbox and the
-- devices are told to take the notification down.
CREATE TABLE pushes (
    message_id UUID PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX pushes_by_user ON pushes(user_id);

CREATE INDEX messages_unread_inbox ON messages(user_id, date DESC)
    WHERE unread AND NOT deleted AND 'inbox' = ANY(labels);
