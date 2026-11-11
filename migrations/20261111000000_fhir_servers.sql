-- Each clinic's FHIR server, set by the operator (I4). One per clinic; the
-- address is checked before it is stored (https, or http on this computer).

create table fhir_servers (
    tenant_id  uuid primary key references accounts (tenant_id) on delete cascade,
    base_url   text not null,
    set_at     timestamptz not null
);
