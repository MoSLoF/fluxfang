-- =====================================================================
-- evidence_state: a graded certainty ladder on every attribution surface,
-- so a guess can no longer masquerade as a confirmed link. Pairs with the
-- existing `source` provenance ('manual'|'auto'|'ai') and the numeric
-- `confidence`/`ai_confidence` already present (0008, 0012).
--
-- The ladder (weakest -> strongest):
--   'hypothesis'  a machine (usually the AI/MCP writer) proposed it on thin
--                 support; must not drive action on its own.
--   'inferred'    the correlation engine derived it heuristically.
--   'observed'    multiple concrete observations back it.
--   'verified'    an operator (or a confirming pass) has confirmed it.
--
-- Design intent: only 'verified' should drive high-severity action (see the
-- alert-rule and MCP-guard changes that follow). AI/MCP writes land at
-- 'hypothesis' and can only be promoted to 'verified' by a human/API action.
--
-- Added to the three attribution surfaces that assert "this belongs with
-- that": emitter_association (emitter<->emitter), entity (the grouped real-
-- world thing), and emitter.entity_id (an emitter's attachment to an entity).
-- The emitter column is NULLable because it is only meaningful when the
-- emitter is actually attached to an entity.
--
-- Backfill is principled, not blanket: existing operator-made rows become
-- 'verified', correlation-engine rows 'inferred', AI rows 'hypothesis' -
-- mirroring the backfill pattern in 0017/0020.
-- See docs/superpowers/specs/ for the certainty-ladder design note.
-- =====================================================================

ALTER TABLE emitter_association
    ADD COLUMN evidence_state text NOT NULL DEFAULT 'inferred'
        CHECK (evidence_state IN ('hypothesis', 'inferred', 'observed', 'verified'));

ALTER TABLE entity
    ADD COLUMN evidence_state text NOT NULL DEFAULT 'inferred'
        CHECK (evidence_state IN ('hypothesis', 'inferred', 'observed', 'verified'));

-- NULL when the emitter is not attached to any entity (entity_id IS NULL).
ALTER TABLE emitter
    ADD COLUMN entity_evidence_state text
        CHECK (entity_evidence_state IN ('hypothesis', 'inferred', 'observed', 'verified'));

-- ---- Backfill from existing provenance -------------------------------

UPDATE emitter_association SET evidence_state =
    CASE source
        WHEN 'manual' THEN 'verified'
        WHEN 'ai'     THEN 'hypothesis'
        ELSE 'inferred'          -- 'auto'
    END;

UPDATE entity SET evidence_state =
    CASE source
        WHEN 'ai' THEN 'hypothesis'
        ELSE 'verified'          -- 'manual'
    END;

UPDATE emitter SET entity_evidence_state =
    CASE
        WHEN entity_id IS NULL THEN NULL
        WHEN source = 'ai'     THEN 'hypothesis'
        ELSE 'verified'          -- 'manual' attachment
    END;

-- Partial index: fast lookups of not-yet-verified attributions (the review
-- queue the operator works down, and the set the verified-gate filters on).
CREATE INDEX emitter_association_unverified_idx
    ON emitter_association (evidence_state)
    WHERE evidence_state <> 'verified';
