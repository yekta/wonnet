-- A subject's words split at a slash: the parser keeps "[Org/Repo]" as one word. The server then
-- rebuilds every message's search.
CREATE OR REPLACE FUNCTION message_search(from_name TEXT, from_email TEXT, recipients JSONB, subject TEXT, words TEXT)
RETURNS tsvector LANGUAGE sql IMMUTABLE AS $$
    SELECT setweight(to_tsvector('simple',
               coalesce(from_name, '') || ' ' || from_email || ' ' || split_part(from_email, '@', 2)), 'A')
        || setweight(to_tsvector('simple', coalesce((
               SELECT string_agg(coalesce(address->>'name', '') || ' ' || (address->>'email') || ' '
                   || split_part(address->>'email', '@', 2), ' ')
               FROM jsonb_array_elements(coalesce(recipients->'to', '[]') || coalesce(recipients->'cc', '[]')
                   || coalesce(recipients->'bcc', '[]')) AS address), '')), 'B')
        || setweight(to_tsvector('simple', translate(subject, '/', ' ')), 'C')
        || to_tsvector('simple', words)
$$;

DELETE FROM search_backfill;
INSERT INTO search_backfill VALUES ('00000000-0000-0000-0000-000000000000');
