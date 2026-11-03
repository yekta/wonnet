-- A mailbox can be in more than one user, like a shared inbox; once in each.
DROP INDEX accounts_by_login;
CREATE UNIQUE INDEX accounts_by_user_login ON accounts(user_id, provider, address, login) WHERE NOT deleted;
CREATE INDEX accounts_by_login ON accounts(provider, address, login) WHERE NOT deleted;
