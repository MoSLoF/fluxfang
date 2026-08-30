-- =====================================================================
-- Add the `external` data source kind (mode `push`): observations arrive
-- over HTTP via `POST /api/data-sources/:id/ingest` and are fed into the
-- normal ingest pipeline by `CaptureSupervisor::ingest_external`, instead
-- of an in-process hardware capturer. This is the door for the RF agent
-- bridge (Marauder/H4M/Flipper → FluxFang).
--
-- Same drop-and-re-add pattern as 0005/0007/0011/0013(sensor). Both CHECKs
-- are rewritten with the FULL current kind set (incl. `sensor`) plus
-- `external`, so no existing kind is dropped. `emission_kind_check` is
-- intentionally NOT touched: pushed observations reuse the existing emission
-- kinds (`wifi`/`bluetooth`/`tpms`); no new emission kind is introduced by
-- this phase. (Phase 2's sub-GHz kinds will need their own migration.)
-- =====================================================================

ALTER TABLE data_source DROP CONSTRAINT data_source_kind_check;
ALTER TABLE data_source
    ADD CONSTRAINT data_source_kind_check
    CHECK (kind IN ('wifi', 'gps', 'bluetooth', 'rtl_sdr', 'sensor', 'external'));

ALTER TABLE data_source DROP CONSTRAINT data_source_kind_mode_check;
ALTER TABLE data_source
    ADD CONSTRAINT data_source_kind_mode_check
    CHECK (
        (kind = 'wifi' AND mode IN ('monitor', 'scan'))
        OR (kind = 'gps' AND mode IN ('gpsd', 'serial', 'manual'))
        OR (kind = 'bluetooth' AND mode = 'scan')
        OR (kind = 'rtl_sdr' AND mode = 'tpms')
        OR (kind = 'sensor' AND mode = 'listener')
        OR (kind = 'external' AND mode = 'push')
    );
