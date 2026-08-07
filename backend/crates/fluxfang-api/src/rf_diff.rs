//! RF-picture window diff (pillar 3): compare the set of emitters observed in
//! two time windows and report what appeared, went dark, or persisted (with
//! deltas). A counter-surveillance primitive - "what changed in the RF picture
//! between then and now" - built entirely from read-only aggregates, so it is
//! safe for autonomous MCP lanes.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use fluxfang_db::{EmissionRepo, WindowEmitter};

/// An emitter present in only one window (newly appeared, or gone dark).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RfEmitter {
    pub emitter_id: Uuid,
    pub name: String,
    pub emitter_type: Option<String>,
    pub emission_count: i64,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

impl From<WindowEmitter> for RfEmitter {
    fn from(w: WindowEmitter) -> Self {
        RfEmitter {
            emitter_id: w.emitter_id,
            name: w.name,
            emitter_type: w.emitter_type,
            emission_count: w.emission_count,
            first_seen: w.first_seen,
            last_seen: w.last_seen,
        }
    }
}

/// An emitter present in both windows, with the change between them.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RfPersistent {
    pub emitter_id: Uuid,
    pub name: String,
    pub emitter_type: Option<String>,
    pub baseline_count: i64,
    pub compare_count: i64,
    pub count_delta: i64,
    pub baseline_avg_dbm: Option<f64>,
    pub compare_avg_dbm: Option<f64>,
}

/// The diff of two RF pictures. `appeared`/`disappeared` are the
/// counter-surveillance signal; `persistent` carries the steady-state set with
/// per-emitter deltas.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RfDiff {
    pub appeared: Vec<RfEmitter>,
    pub disappeared: Vec<RfEmitter>,
    pub persistent: Vec<RfPersistent>,
}

/// Pure set-diff of two window summaries, keyed by emitter id. No I/O, so it is
/// unit-testable without a database.
pub fn diff_windows(baseline: Vec<WindowEmitter>, compare: Vec<WindowEmitter>) -> RfDiff {
    let base_ids: HashMap<Uuid, WindowEmitter> =
        baseline.into_iter().map(|w| (w.emitter_id, w)).collect();
    let mut compare_by_id: HashMap<Uuid, WindowEmitter> =
        compare.into_iter().map(|w| (w.emitter_id, w)).collect();

    let mut appeared: Vec<RfEmitter> = Vec::new();
    let mut disappeared: Vec<RfEmitter> = Vec::new();
    let mut persistent: Vec<RfPersistent> = Vec::new();

    for (id, base) in base_ids {
        match compare_by_id.remove(&id) {
            Some(cmp) => persistent.push(RfPersistent {
                emitter_id: id,
                name: cmp.name.clone(),
                emitter_type: cmp.emitter_type.clone(),
                baseline_count: base.emission_count,
                compare_count: cmp.emission_count,
                count_delta: cmp.emission_count - base.emission_count,
                baseline_avg_dbm: base.avg_dbm,
                compare_avg_dbm: cmp.avg_dbm,
            }),
            None => disappeared.push(base.into()),
        }
    }
    // Whatever is left in compare_by_id was not in baseline -> newly appeared.
    for (_id, cmp) in compare_by_id {
        appeared.push(cmp.into());
    }

    // Stable ordering for deterministic output/tests.
    appeared.sort_by(|a, b| a.name.cmp(&b.name));
    disappeared.sort_by(|a, b| a.name.cmp(&b.name));
    persistent.sort_by(|a, b| a.name.cmp(&b.name));

    RfDiff {
        appeared,
        disappeared,
        persistent,
    }
}

/// Fetch both window summaries and diff them.
pub async fn run_rf_diff(
    pool: &PgPool,
    baseline_from: DateTime<Utc>,
    baseline_to: DateTime<Utc>,
    compare_from: DateTime<Utc>,
    compare_to: DateTime<Utc>,
    kind: Option<&str>,
) -> Result<RfDiff, sqlx::Error> {
    let baseline = EmissionRepo::window_emitter_summary(pool, baseline_from, baseline_to, kind).await?;
    let compare = EmissionRepo::window_emitter_summary(pool, compare_from, compare_to, kind).await?;
    Ok(diff_windows(baseline, compare))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn we(name: &str, count: i64) -> WindowEmitter {
        let now = Utc::now();
        WindowEmitter {
            emitter_id: Uuid::new_v4(),
            name: name.into(),
            emitter_type: None,
            emission_count: count,
            first_seen: now,
            last_seen: now,
            min_dbm: None,
            max_dbm: None,
            avg_dbm: None,
        }
    }

    #[test]
    fn diff_partitions_appeared_disappeared_persistent() {
        let stays = we("stays", 3);
        let mut stays_later = stays.clone();
        stays_later.emission_count = 7; // +4 delta

        let gone = we("gone", 2); // baseline only
        let new = we("new", 5); // compare only

        let baseline = vec![stays.clone(), gone.clone()];
        let compare = vec![stays_later.clone(), new.clone()];

        let d = diff_windows(baseline, compare);

        assert_eq!(d.appeared.len(), 1);
        assert_eq!(d.appeared[0].emitter_id, new.emitter_id);

        assert_eq!(d.disappeared.len(), 1);
        assert_eq!(d.disappeared[0].emitter_id, gone.emitter_id);

        assert_eq!(d.persistent.len(), 1);
        assert_eq!(d.persistent[0].emitter_id, stays.emitter_id);
        assert_eq!(d.persistent[0].count_delta, 4);
    }

    #[test]
    fn empty_windows_diff_to_nothing() {
        let d = diff_windows(vec![], vec![]);
        assert!(d.appeared.is_empty() && d.disappeared.is_empty() && d.persistent.is_empty());
    }
}
