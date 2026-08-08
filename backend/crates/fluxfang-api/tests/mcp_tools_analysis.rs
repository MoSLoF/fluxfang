use chrono::{TimeZone, Utc};
use serde_json::json;

mod common;
use common::fresh_pool_shared as fresh_pool;

use fluxfang_api::mcp::tools::analysis;
use fluxfang_db::models::{NewDataSource, NewEmission, NewEmitter};
use fluxfang_db::{DataSourceRepo, EmissionRepo, EmitterRepo, SessionRepo};

#[tokio::test]
async fn collocation_counts_cooccurring_pairs() {
    let pool = fresh_pool().await;
    let ds = DataSourceRepo::insert(&pool, NewDataSource::wifi_monitor("wlan0")).await.unwrap().id;
    SessionRepo::close_active(&pool).await.ok();
    let session = SessionRepo::open(&pool).await.unwrap().id;

    let a = EmitterRepo::insert(&pool, NewEmitter { name: "A".into(), ..Default::default() }).await.unwrap().id;
    let b = EmitterRepo::insert(&pool, NewEmitter { name: "B".into(), ..Default::default() }).await.unwrap().id;
    let base = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    // Two near-simultaneous observations per emitter, in two clusters.
    for (secs, emitter) in [(0i64, a), (1, b), (300, a), (301, b)] {
        let mut em = NewEmission::wifi(ds, session, json!({"x": 1}));
        em.emitter_id = Some(emitter);
        em.observed_at = base + chrono::Duration::seconds(secs);
        EmissionRepo::insert(&pool, em).await.unwrap();
    }

    let out = analysis::collocation_query(&pool, json!({
        "emitter_ids": [a.to_string(), b.to_string()], "window_seconds": 60
    })).await.unwrap();
    let pairs = out["pairs"].as_array().unwrap();
    assert_eq!(pairs.len(), 1);
    assert!(pairs[0]["cooccurrences"].as_i64().unwrap() >= 2);
}

#[tokio::test]
async fn suggest_associations_returns_verdicts() {
    let pool = fresh_pool().await;
    let ds = DataSourceRepo::insert(&pool, NewDataSource::wifi_monitor("wlan0")).await.unwrap().id;
    SessionRepo::close_active(&pool).await.ok();
    let session = SessionRepo::open(&pool).await.unwrap().id;
    let a = EmitterRepo::insert(&pool, NewEmitter { name: "A".into(), ..Default::default() }).await.unwrap().id;
    let b = EmitterRepo::insert(&pool, NewEmitter { name: "B".into(), ..Default::default() }).await.unwrap().id;
    let base = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    // Co-located far-apart events → geographic verdict (0.9).
    for (secs, emitter, lon, lat) in [
        (0i64, a, 0.0, 0.0), (1, b, 0.0, 0.0),
        (600, a, 0.2, 0.0), (601, b, 0.2, 0.0),
    ] {
        let mut em = NewEmission::wifi(ds, session, json!({}));
        em.emitter_id = Some(emitter);
        em.observed_at = base + chrono::Duration::seconds(secs);
        em.location = Some((lon, lat));
        EmissionRepo::insert(&pool, em).await.unwrap();
    }

    let out = analysis::suggest_associations(&pool, json!({
        "emitter_ids": [a.to_string(), b.to_string()]
    })).await.unwrap();
    let s = out["suggestions"].as_array().unwrap();
    assert_eq!(s.len(), 1);
    assert!(s[0]["confidence"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn diff_rf_picture_reports_appeared_disappeared_persistent() {
    let pool = fresh_pool().await;
    let ds = DataSourceRepo::insert(&pool, NewDataSource::wifi_monitor("wlan0"))
        .await
        .unwrap()
        .id;
    SessionRepo::close_active(&pool).await.ok();
    let session = SessionRepo::open(&pool).await.unwrap().id;

    let gone = EmitterRepo::insert(&pool, NewEmitter { name: "Gone".into(), ..Default::default() })
        .await
        .unwrap()
        .id;
    let stays = EmitterRepo::insert(&pool, NewEmitter { name: "Stays".into(), ..Default::default() })
        .await
        .unwrap()
        .id;
    let arrived =
        EmitterRepo::insert(&pool, NewEmitter { name: "Arrived".into(), ..Default::default() })
            .await
            .unwrap()
            .id;

    let base = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    // Baseline window [base, base+60): gone(1), stays(2).
    for (secs, emitter) in [(0i64, gone), (5, stays), (6, stays)] {
        let mut em = NewEmission::wifi(ds, session, json!({}));
        em.emitter_id = Some(emitter);
        em.observed_at = base + chrono::Duration::seconds(secs);
        EmissionRepo::insert(&pool, em).await.unwrap();
    }
    // Compare window [base+3600, +60): stays(3), arrived(1).
    let c0 = base + chrono::Duration::seconds(3600);
    for (secs, emitter) in [(0i64, stays), (1, stays), (2, stays), (3, arrived)] {
        let mut em = NewEmission::wifi(ds, session, json!({}));
        em.emitter_id = Some(emitter);
        em.observed_at = c0 + chrono::Duration::seconds(secs);
        EmissionRepo::insert(&pool, em).await.unwrap();
    }

    let out = analysis::diff_rf_picture(
        &pool,
        json!({
            "baseline_from": base.to_rfc3339(),
            "baseline_to": (base + chrono::Duration::seconds(60)).to_rfc3339(),
            "compare_from": c0.to_rfc3339(),
            "compare_to": (c0 + chrono::Duration::seconds(60)).to_rfc3339(),
        }),
    )
    .await
    .unwrap();

    let appeared = out["appeared"].as_array().unwrap();
    let disappeared = out["disappeared"].as_array().unwrap();
    let persistent = out["persistent"].as_array().unwrap();

    assert_eq!(appeared.len(), 1);
    assert_eq!(appeared[0]["emitter_id"].as_str().unwrap(), arrived.to_string());
    assert_eq!(disappeared.len(), 1);
    assert_eq!(disappeared[0]["emitter_id"].as_str().unwrap(), gone.to_string());
    assert_eq!(persistent.len(), 1);
    assert_eq!(persistent[0]["emitter_id"].as_str().unwrap(), stays.to_string());
    assert_eq!(persistent[0]["count_delta"].as_i64().unwrap(), 1);
}
