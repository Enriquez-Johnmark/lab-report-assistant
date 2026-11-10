-- The physician's review: who may approve, the warnings shown on a
-- physician's own edits, and each approval with the exact text's hash.

alter table drafts add column warnings jsonb not null default '[]';

-- Members of a clinic who may approve reports (C1). The administrator is the
-- account's owner and is never listed here.
create table physicians (
    member_id   uuid primary key,
    tenant_id   uuid not null,
    name        text not null check (length(name) between 1 and 100),
    created_at  timestamptz not null,
    foreign key (member_id, tenant_id) references account_members (id, tenant_id) on delete cascade
);

-- An approval binds the draft's exact text by its hash (C3).
create table approvals (
    draft_id       uuid primary key,
    tenant_id      uuid not null,
    lab_report_id  uuid not null,
    content_hash   text not null,
    approved_by    uuid not null,
    approved_at    timestamptz not null,
    foreign key (tenant_id, draft_id) references drafts (tenant_id, id) on delete cascade,
    foreign key (tenant_id, lab_report_id) references lab_reports (tenant_id, id) on delete cascade
);
