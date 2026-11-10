-- Drafts of a lab report's patient report: written by the model, from the
-- fixed template, or by a physician, each with the checks it failed.

create table drafts (
    id             uuid primary key,
    tenant_id      uuid not null,
    lab_report_id  uuid not null,
    -- 1 for the first draft of a lab report, then 2, 3…
    attempt        integer not null check (attempt >= 1),
    source         text not null check (source in ('model', 'template', 'physician')),
    model          text,
    explanations   jsonb not null,
    failures       jsonb not null,
    blocked        boolean not null,
    created_at     timestamptz not null,
    unique (lab_report_id, attempt),
    unique (tenant_id, id),
    foreign key (tenant_id, lab_report_id) references lab_reports (tenant_id, id) on delete cascade
);
