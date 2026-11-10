-- Each approved report's delivery: the notice email in the engine's outbox
-- and the signed link it carries (D1–D5). The link names the delivery, which
-- names the exact draft approved.

create table deliveries (
    id             uuid primary key,
    tenant_id      uuid not null,
    lab_report_id  uuid not null,
    draft_id       uuid not null,
    message_id     uuid not null unique,
    created_at     timestamptz not null,
    -- The link stops working after this (D4).
    expires_at     timestamptz not null,
    unique (tenant_id, id),
    foreign key (tenant_id, draft_id) references drafts (tenant_id, id) on delete cascade,
    foreign key (tenant_id, lab_report_id) references lab_reports (tenant_id, id) on delete cascade
);
