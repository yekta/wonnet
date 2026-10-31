-- A sign-in's PKCE verifier, for Google or Microsoft.
ALTER TABLE sign_ins RENAME COLUMN google_verifier TO verifier;
