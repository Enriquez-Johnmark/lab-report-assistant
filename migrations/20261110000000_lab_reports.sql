-- Patients, lab reports and their results, each owned by one clinic (the
-- engine's account). Erasing a clinic's account removes them with it.

create table patients (
    id          uuid primary key,
    tenant_id   uuid not null references accounts (tenant_id) on delete cascade,
    fhir_id     text not null,
    name        text not null,
    email       text,
    created_at  timestamptz not null,
    unique (tenant_id, fhir_id),
    unique (tenant_id, id)
);

create table lab_reports (
    id           uuid primary key,
    tenant_id    uuid not null references accounts (tenant_id) on delete cascade,
    patient_id   uuid not null,
    fhir_id      text not null,
    name         text not null,
    effective    text,
    imported_at  timestamptz not null,
    unique (tenant_id, fhir_id),
    unique (tenant_id, id),
    -- A report's patient belongs to the same clinic.
    foreign key (tenant_id, patient_id) references patients (tenant_id, id) on delete cascade
);

create table lab_results (
    tenant_id      uuid not null,
    lab_report_id  uuid not null,
    position       integer not null,
    fhir_id        text not null,
    code           text not null,
    name           text not null,
    value          double precision not null,
    unit           text not null,
    range_low      double precision,
    range_high     double precision,
    flag           text not null check (flag in ('low', 'normal', 'high')),
    primary key (lab_report_id, position),
    check (range_low is not null or range_high is not null),
    foreign key (tenant_id, lab_report_id) references lab_reports (tenant_id, id) on delete cascade
);
