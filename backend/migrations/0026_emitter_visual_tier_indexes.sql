-- Visual-tier read model: index the emitter summary the map renders from, so
-- GET /api/emitters/map can filter by activity window (last_seen_at) + bbox
-- (est_location) instead of scanning the ~10.9M-row emission table.
-- emitter is ~19k rows, so a plain (non-concurrent) build is effectively instant.
CREATE INDEX IF NOT EXISTS emitter_last_seen_idx ON emitter (last_seen_at DESC);
CREATE INDEX IF NOT EXISTS emitter_est_location_gist ON emitter USING gist (est_location);
