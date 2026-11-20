-- A search in one account walks that account's mail newest first, not all of the user's.
CREATE INDEX messages_by_account_date ON messages(account_id, date DESC) WHERE NOT deleted;
