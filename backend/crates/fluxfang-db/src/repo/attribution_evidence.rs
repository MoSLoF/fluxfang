//! `attribution_evidence`: append-only, FK-free witness snapshots captured at
//! the moment an attribution decision is made (migration
//! `0023_attribution_evidence.sql`). Rows are written once via [`record`] and
//! never updated; the justification for a decision is meant to outlive the
//! emitter/entity it refers to, so the snapshot survives even after the AI
//! deletes the underlying rows.
//!
//! [`record`]: AttributionEvidenceRepo::record

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{AttributionEvidence, NewAttributionEvidence};

/// Column list shared by every query that produces an [`AttributionEvidence`].
const ATTRIBUTION_EVIDENCE_COLUMNS: &str = "id, created_at, subject_kind, subject_a, subject_b, \
     asserted_by, evidence_state, confidence, witness, complete";

pub struct AttributionEvidenceRepo;

impl AttributionEvidenceRepo {
    /// Persist a witness snapshot. `complete` is left to the DB default
    /// (`true`); everything else is taken verbatim from `new`.
    pub async fn record(
        pool: &PgPool,
        new: NewAttributionEvidence,
    ) -> Result<AttributionEvidence, sqlx::Error> {
        let sql = format!(
            "INSERT INTO attribution_evidence \
             (subject_kind, subject_a, subject_b, asserted_by, evidence_state, confidence, witness) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             RETURNING {ATTRIBUTION_EVIDENCE_COLUMNS}"
        );
        sqlx::query_as::<_, AttributionEvidence>(&sql)
            .bind(new.subject_kind)
            .bind(new.subject_a)
            .bind(new.subject_b)
            .bind(new.asserted_by)
            .bind(new.evidence_state)
            .bind(new.confidence)
            .bind(new.witness)
            .fetch_one(pool)
            .await
    }

    /// All witness snapshots for a subject, newest first.
    ///
    /// When `subject_b` is `Some`, the pair is matched in either stored
    /// direction (association subjects are recorded once as passed, but a
    /// caller may look them up by either emitter). When `subject_b` is `None`,
    /// only rows with a NULL `subject_b` (single-subject kinds, e.g. `entity`)
    /// match.
    pub async fn list_for_subject(
        pool: &PgPool,
        subject_a: Uuid,
        subject_b: Option<Uuid>,
    ) -> Result<Vec<AttributionEvidence>, sqlx::Error> {
        let sql = if subject_b.is_some() {
            format!(
                "SELECT {ATTRIBUTION_EVIDENCE_COLUMNS} FROM attribution_evidence \
                 WHERE (subject_a = $1 AND subject_b = $2) \
                    OR (subject_a = $2 AND subject_b = $1) \
                 ORDER BY created_at DESC"
            )
        } else {
            format!(
                "SELECT {ATTRIBUTION_EVIDENCE_COLUMNS} FROM attribution_evidence \
                 WHERE subject_a = $1 AND subject_b IS NULL \
                 ORDER BY created_at DESC"
            )
        };
        let mut q = sqlx::query_as::<_, AttributionEvidence>(&sql).bind(subject_a);
        if let Some(b) = subject_b {
            q = q.bind(b);
        }
        q.fetch_all(pool).await
    }
}
