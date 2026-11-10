-- The FHIR file a lab report was imported from, exactly as it arrived, so
-- the clinic can see where every value came from.
alter table lab_reports add column source_fhir text;
