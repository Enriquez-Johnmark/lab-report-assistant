-- A draft the physician rejected, asking for a new one (C6). It can't be
-- approved, and no other new draft is asked for until the new one is written.
alter table drafts add column rejected_at timestamptz;
