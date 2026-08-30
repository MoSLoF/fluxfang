-- =====================================================================
-- Add `subghz` as an allowed emission kind (Phase 2 of the RF agent bridge:
-- Flipper Zero / HackRF sub-GHz decodes — keyfobs, garage/gate remotes,
-- weather stations, generic OOK/FSK devices in the 300–928 MHz band).
--
-- Only `emission_kind_check` widens. `fluxfang_core::classify` has no `subghz`
-- arm, so these emissions are stored but left STRAY (unassigned) — the analyst
-- groups them into emitters via match rules (`set_emitter_match_rule`) keyed on
-- a decoded id/serial in the payload. A future `classify_subghz` could
-- auto-create emitters the way `classify_tpms` does; not part of this phase.
--
-- No `data_source` constraint changes: sub-GHz arrives through the existing
-- `external`/`push` source (kind stays `external`); only the emission's own
-- `kind` field is new.
-- =====================================================================

ALTER TABLE emission DROP CONSTRAINT emission_kind_check;
ALTER TABLE emission
    ADD CONSTRAINT emission_kind_check
    CHECK (kind IN ('wifi', 'bluetooth', 'tpms', 'subghz'));
