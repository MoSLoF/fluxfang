# Attribution Evidence Discipline

Status: proposed (pillar 1 implemented; pillars 2-4 scoped as follow-ups)

## Problem

FluxFang makes attribution claims - "these emitters are the same vehicle",
"this emitter belongs to that entity" - and today those claims carry only a
`source` provenance tag (`manual`/`auto`/`ai`) and, on `emitter_association`, a
bare numeric `confidence`. Two gaps follow:

1. **A guess reads like a fact.** A weak time-only auto-correlation and an
   operator-confirmed link are indistinguishable in the API and UI. SIGINT
   attribution is exactly where overclaim is dangerous.
2. **The AI has a delete key and no discipline.** The MCP writer has full,
   destructive, no-undo authority over emitters and entities. Nothing stops an
   AI-proposed attribution from being treated as ground truth, or preserves
   *why* a link was believed once the rows are gone.

## Approach

Borrowing the evidence-state discipline used in graph-based attack-path tooling
(BloodHound / AgentHound-style findings, where a claim is never scored as truth
until a `verified` state), four pillars:

1. **Grade certainty** - a graded `evidence_state` ladder on every attribution
   surface.
2. **Snapshot evidence** - a frozen, append-only witness record captured at
   decision time that outlives the row it justified.
3. **Diff over time** - fingerprint an RF picture per window and diff two
   windows (a read-only counter-surveillance primitive).
4. **Verified drives action** - gate AI writes and high-severity alerting on
   evidence_state.

### The ladder

`hypothesis` -> `inferred` -> `observed` -> `verified` (weakest to strongest).
Initial state is *derived*, not hand-entered: `manual` -> `verified`,
`ai` -> `hypothesis`, `auto` -> graded by correlation confidence
(>= 0.8 geographic -> `observed`, time-only fallback -> `inferred`). Deriving it
inside the insert/add methods means no `NewEntity`/`add()` call site changes.

## What this PR contains

- `migrations/0022_evidence_state.sql` - `evidence_state` on
  `emitter_association`, `entity`, and (nullable) `emitter.entity_evidence_state`,
  with a principled backfill (mirroring the 0017/0020 pattern) and a partial
  index over the not-yet-verified set.
- `migrations/0023_attribution_evidence.sql` - the frozen witness table.
- **Pillar 1**: `evidence_state` on the `Entity`/`AssociatedEmitter` models,
  derived-at-write in `EntityRepo::insert` + `EmitterAssociationRepo::add`,
  surfaced via `EntityDto`/`EmitterAssociationDto`.
- **Pillar 2 (repo + auto-correlation capture)**: an `AttributionEvidence`
  model + append-only `AttributionEvidenceRepo` (`record`/`list_for_subject`),
  and witness capture in the TPMS auto-correlation pass - every `auto`
  association now freezes the co-occurrence basis (geometry, timing, span,
  confidence) at decision time. A shared `initial_evidence_state` helper backs
  entity insert, association add, and this capture so the grading can't drift.
- **Pillar 4 (verified-gate on the AI)**: AI writes already land at `hypothesis`
  (the source-derived grading from pillar 1), so the AI cannot self-assert a
  higher state. The destructive side is now gated too: the MCP `unlink_emitters`
  and `delete_entity` tools refuse to remove a `verified` (operator-confirmed)
  attribution, returning a `Forbidden` tool error the AI Audit Log records. This
  is what neutralizes the "destructive, no-undo" authority - the AI can retract
  its own guesses but cannot destroy confirmed truth.

### Frozen-snapshot rationale (0023)

`attribution_evidence` deliberately has no foreign keys and no cascade. The AI
can delete the row a record refers to; the point is that the justification
survives that deletion. `ai_audit_log` (0012) records *that* something changed;
this records *why it was believed*. Written once, never updated.

## Follow-ups (not in this PR)

- **Pillar 2 (remaining)**: extend witness capture to the manual and AI/MCP
  write paths (`mcp/tools/writes.rs`, `emitters.rs`), and grade the
  `emitter.entity_id` attachment via the nullable column 0022 adds.
- **Pillar 3**: RF-picture fingerprint + window diff (`core/rf_diff.rs`, an API
  route, a read-only `diff_rf_picture` MCP analysis tool).
- **Pillar 4 (remaining)**: an operator promotion path (`hypothesis` ->
  `verified`) via the API/UI (verified currently only arises from manual
  creation), and an evidence_state filter on alert rules so only `verified`
  attributions fire high-severity alerts.

## Review notes

- Repos use runtime-checked `query_as` (string SQL) - no offline `.sqlx`
  regeneration needed; changes validate at test time against the migrated schema.
- New fields appear in `entity`/association JSON; any exact-shape response
  assertion needs its expected value updated. No method signatures changed.
- The 0.8 auto-grading threshold is inlined for now; it could move to
  `CorrelationConfig` alongside the existing correlation thresholds.
