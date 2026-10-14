-- The server makes the snippets of the bodies it has again, and their search, a batch at a time in
-- message id order, after the id kept here; the row goes once it is done.
CREATE TABLE snippet_backfill (after UUID NOT NULL);
INSERT INTO snippet_backfill VALUES ('00000000-0000-0000-0000-000000000000');
