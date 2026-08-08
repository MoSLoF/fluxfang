-- =====================================================================
-- attribution_evidence: a frozen witness snapshot captured at the moment an
-- attribution decision is made - the emissions, signal levels, and
-- correlation basis that justified "this belongs with that" - so a later
-- review sees the evidence AS IT WAS at decision time, not a re-derived view
-- of mutated data.
--
-- KEY DESIGN CHOICE: no foreign keys on the subject columns, and nothing
-- cascades. This is deliberate. FluxFang's MCP writer has full destructive,
-- no-undo authority over emitters/entities; the whole point of this table is
-- that the justification for a decision OUTLIVES the row it justified. If the
-- AI later deletes an emitter or detaches an entity, the append-only
-- ai_audit_log (0012) records THAT it changed, and this table records WHY the
-- link was believed in the first place. A cascade would erase exactly the
-- forensic record we are trying to keep. Rows here are written once and never
-- updated.
--
-- `subject_kind` selects what the two subject columns mean (polymorphic ref,
-- hence no FK): for 'emitter_association' they are the two emitter ids; for
-- 'emitter_entity' they are (emitter_id, entity_id); for 'entity' only
-- subject_a is set. `evidence_state`/`asserted_by`/`confidence` snapshot the
-- grading at decision time (they can drift on the live row afterward). The
-- `witness` jsonb holds the verbatim basis: observed emission ids, signal
-- levels, collocation/timing/distance findings, and the observation window.
-- =====================================================================

CREATE TABLE attribution_evidence (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at     timestamptz NOT NULL DEFAULT now(),

    subject_kind   text NOT NULL
        CHECK (subject_kind IN ('emitter_association', 'emitter_entity', 'entity')),
    subject_a      uuid NOT NULL,          -- emitter_id, or entity_id for 'entity'
    subject_b      uuid,                    -- associated_emitter_id / entity_id; NULL for 'entity'

    asserted_by    text NOT NULL
        CHECK (asserted_by IN ('manual', 'auto', 'ai')),
    evidence_state text NOT NULL
        CHECK (evidence_state IN ('hypothesis', 'inferred', 'observed', 'verified')),
    confidence     double precision,

    -- Verbatim basis, frozen at decision time. Shape (by convention, not
    -- enforced): { "emission_ids": [...], "signal_dbm": [...],
    -- "basis": {"collocation": ..., "timing": ..., "distance_m": ...},
    -- "window": {"from": "...", "to": "..."} }
    witness        jsonb NOT NULL DEFAULT '{}'::jsonb,
    complete       boolean NOT NULL DEFAULT true
);

CREATE INDEX attribution_evidence_subject_idx
    ON attribution_evidence (subject_a, subject_b);
CREATE INDEX attribution_evidence_created_at_idx
    ON attribution_evidence (created_at DESC);
