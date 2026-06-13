CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE users (
  id uuid PRIMARY KEY,
  google_sub text NOT NULL UNIQUE,
  email text NOT NULL UNIQUE,
  name text,
  picture_url text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE user_sessions (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_hash text NOT NULL UNIQUE,
  expires_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE floorplans (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title text NOT NULL,
  status text NOT NULL,
  source_filename text NOT NULL,
  source_size_bytes bigint NOT NULL,
  source_sha256 text NOT NULL,
  source_artifact_path text NOT NULL,
  floorplan_json_path text,
  svg_path text,
  pdf_path text,
  thumbnail_path text,
  confidence double precision NOT NULL DEFAULT 0,
  total_area_sqft double precision,
  width_ft double precision,
  depth_ft double precision,
  failure_reason text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX floorplans_user_created_idx ON floorplans(user_id, created_at DESC);

CREATE TABLE processing_jobs (
  id uuid PRIMARY KEY,
  floorplan_id uuid NOT NULL UNIQUE REFERENCES floorplans(id) ON DELETE CASCADE,
  status text NOT NULL,
  progress integer NOT NULL DEFAULT 0,
  step text NOT NULL DEFAULT 'Queued',
  error text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE monthly_save_events (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  floorplan_id uuid NOT NULL UNIQUE REFERENCES floorplans(id) ON DELETE CASCADE,
  month_start date NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX monthly_save_events_user_month_idx ON monthly_save_events(user_id, month_start);

CREATE TABLE artifact_objects (
  id uuid PRIMARY KEY,
  floorplan_id uuid NOT NULL REFERENCES floorplans(id) ON DELETE CASCADE,
  kind text NOT NULL,
  path text NOT NULL,
  content_type text NOT NULL,
  size_bytes bigint NOT NULL,
  sha256 text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX artifact_objects_floorplan_idx ON artifact_objects(floorplan_id);
