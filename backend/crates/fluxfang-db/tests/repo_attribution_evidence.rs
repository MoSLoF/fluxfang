//! Round-trip tests for `AttributionEvidenceRepo` (pillar 2).

mod common;

use common::fresh_pool;
use fluxfang_db::models::NewAttributionEvidence;
use fluxfang_db::AttributionEvidenceRepo;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn record_and_list_for_subject_roundtrips_witness_from_either_direction() {
    let pool = fresh_pool().await;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    let rec = AttributionEvidenceRepo::record(
        &pool,
        NewAttributionEvidence {
            subject_kind: "emitter_association".into(),
            subject_a: a,
            subject_b: Some(b),
            asserted_by: "auto".into(),
            evidence_state: "observed".into(),
            confidence: Some(0.9),
            witness: json!({ "basis": "geographic", "max_separation_m": 1600.0 }),
        },
    )
    .await
    .unwrap();

    assert!(!rec.id.is_nil());
    assert!(rec.complete, "complete must default to true");
    assert_eq!(rec.witness["basis"], "geographic");

    // Listable by either emitter of the pair, same row.
    let by_a = AttributionEvidenceRepo::list_for_subject(&pool, a, Some(b))
        .await
        .unwrap();
    let by_b = AttributionEvidenceRepo::list_for_subject(&pool, b, Some(a))
        .await
        .unwrap();
    assert_eq!(by_a.len(), 1);
    assert_eq!(by_b.len(), 1);
    assert_eq!(by_a[0].id, by_b[0].id);
    assert_eq!(by_a[0].evidence_state, "observed");
}

#[tokio::test]
async fn snapshot_survives_subjects_that_do_not_exist() {
    // No FK / no cascade is the whole point: recording (and reading) evidence
    // about ids that are not - or are no longer - real rows must succeed, so
    // the justification outlives an AI delete.
    let pool = fresh_pool().await;
    let ghost = Uuid::new_v4();

    let rec = AttributionEvidenceRepo::record(
        &pool,
        NewAttributionEvidence {
            subject_kind: "entity".into(),
            subject_a: ghost,
            subject_b: None,
            asserted_by: "ai".into(),
            evidence_state: "hypothesis".into(),
            confidence: Some(0.5),
            witness: json!({ "note": "AI-proposed, unverified" }),
        },
    )
    .await
    .unwrap();

    let got = AttributionEvidenceRepo::list_for_subject(&pool, rec.subject_a, None)
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].asserted_by, "ai");
    assert_eq!(got[0].subject_b, None);
}
