//! The periodic TPMS correlation pass (Spec B): loads candidate tpms_sensor
//! emitters (those from an auto-correlate data source), and for each
//! same-model pair not already linked, runs the pure engine
//! (`fluxfang_core::correlate`) over their recent emissions and adds an
//! `auto` association on a positive verdict. Only ever ADDS `source='auto'`.

use std::time::Duration;

use chrono::{DateTime, Utc};
use fluxfang_core::correlate::{
    cooccurrences, haversine_meters, should_associate, CoEvent, CorrelationConfig, Reading,
};
use fluxfang_db::models::{initial_evidence_state, Emitter, NewAttributionEvidence};
use fluxfang_db::repo::emission::EmissionFilter;
use fluxfang_db::{AttributionEvidenceRepo, EmissionRepo, EmitterAssociationRepo, EmitterRepo};
use serde_json::json;
use sqlx::PgPool;

/// How far back to pull emissions when correlating.
const LOOKBACK: Duration = Duration::from_secs(60 * 60 * 24); // 24h
/// Max emissions to consider per emitter per pass.
const MAX_READINGS: i64 = 1000;

/// Run one correlation pass. Returns the number of new associations added.
pub async fn run_correlation_pass(pool: &PgPool, now: DateTime<Utc>) -> anyhow::Result<usize> {
    let cfg = CorrelationConfig::default();
    let time_from = now - chrono::Duration::from_std(LOOKBACK)?;
    let candidates = EmitterRepo::list_auto_correlate_tpms(pool, time_from).await?;

    // Fetch each candidate's recent readings once.
    let mut readings: Vec<(Emitter, Vec<Reading>)> = Vec::new();
    for e in candidates {
        let filter = EmissionFilter {
            emitter_id: Some(e.id),
            time_from: Some(time_from),
            kind: Some("tpms".to_string()),
            limit: MAX_READINGS,
            ..Default::default()
        };
        let (emissions, _) = EmissionRepo::query(pool, filter).await?;
        let rs = emissions
            .into_iter()
            .map(|em| Reading {
                at: em.observed_at,
                lon: em.lon,
                lat: em.lat,
            })
            .collect();
        readings.push((e, rs));
    }

    let mut added = 0usize;
    for i in 0..readings.len() {
        for j in (i + 1)..readings.len() {
            let (ea, ra) = (&readings[i].0, &readings[i].1);
            let (eb, rb) = (&readings[j].0, &readings[j].1);

            let models_match = model_of(ea) == model_of(eb) && model_of(ea).is_some();
            if !models_match {
                continue;
            }
            if EmitterAssociationRepo::exists(pool, ea.id, eb.id).await? {
                continue;
            }
            let events = cooccurrences(ra, rb, cfg.cooccur_window);
            if let Some(confidence) = should_associate(&events, true, &cfg) {
                EmitterAssociationRepo::add(pool, ea.id, eb.id, "auto", Some(confidence)).await?;
                // Pillar 2: freeze the co-occurrence basis that justified this
                // link, at decision time, so the "why" survives later mutation.
                AttributionEvidenceRepo::record(
                    pool,
                    NewAttributionEvidence {
                        subject_kind: "emitter_association".to_string(),
                        subject_a: ea.id,
                        subject_b: Some(eb.id),
                        asserted_by: "auto".to_string(),
                        evidence_state: initial_evidence_state("auto", Some(confidence))
                            .to_string(),
                        confidence: Some(confidence),
                        witness: build_witness(&events, confidence, &cfg),
                    },
                )
                .await?;
                added += 1;
            }
        }
    }
    Ok(added)
}

fn model_of(e: &Emitter) -> Option<String> {
    e.attributes
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// Freeze the co-occurrence basis behind an auto-association into a witness
/// JSON, captured at decision time (pillar 2). Bounded in size: at most 50
/// events are inlined. `basis` mirrors `should_associate`'s two branches
/// (geographic vs. time-only), and `max_separation_m` is the widest located
/// pair - the geographic evidence for a >= 1 mile co-travel.
fn build_witness(events: &[CoEvent], confidence: f64, cfg: &CorrelationConfig) -> serde_json::Value {
    let basis = if confidence >= 0.8 {
        "geographic"
    } else {
        "time_fallback"
    };
    let located: Vec<&CoEvent> = events
        .iter()
        .filter(|e| e.lon.is_some() && e.lat.is_some())
        .collect();
    let mut max_separation_m = 0.0f64;
    for (i, e1) in located.iter().enumerate() {
        for e2 in &located[i + 1..] {
            let d = haversine_meters(
                e1.lon.unwrap(),
                e1.lat.unwrap(),
                e2.lon.unwrap(),
                e2.lat.unwrap(),
            );
            if d > max_separation_m {
                max_separation_m = d;
            }
        }
    }
    let sample: Vec<serde_json::Value> = events
        .iter()
        .take(50)
        .map(|e| json!({ "at": e.at, "lon": e.lon, "lat": e.lat }))
        .collect();
    json!({
        "basis": basis,
        "confidence": confidence,
        "event_count": events.len(),
        "located_event_count": located.len(),
        "max_separation_m": max_separation_m,
        "mile_meters_threshold": cfg.mile_meters,
        "window": { "from": events.iter().map(|e| e.at).min(), "to": events.iter().map(|e| e.at).max() },
        "events": sample,
    })
}
