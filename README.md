# GLB to Floorplan

A greenfield Vite/React frontend plus Rust `axum` backend for converting real-space `.glb` scans into measured floorplan SVG/PDF artifacts.

## What Works Now

- Drag/drop or file-picker upload UI.
- Google sign-in wiring with a local dev auth fallback.
- Rust API for auth, account quota, GLB upload, processing progress, saved floorplans, SVG preview, and PDF export.
- Postgres schema for users, sessions, floorplans, jobs, save events, and artifact records.
- GLB/glTF parsing with transform accumulation and scale validation.
- Deterministic v1 floorplan generation from scan bounds, semantic node-name hints, measured SVG, and a vector PDF.
- 5 completed floorplans/month quota; PDF re-downloads do not consume quota.

## Local Development

1. Start Postgres:

   ```bash
   docker compose up -d postgres
   ```

2. Copy env values:

   ```bash
   cp .env.example .env
   ```

3. Install frontend dependencies:

   ```bash
   npm install
   ```

4. Run backend migrations and server:

   ```bash
   cd backend
   PATH="$HOME/.cargo/bin:$PATH" sqlx migrate run
   PATH="$HOME/.cargo/bin:$PATH" cargo run
   ```

5. Run frontend:

   ```bash
   npm run dev
   ```

Open `http://localhost:5173`.

If `GOOGLE_CLIENT_ID` and `VITE_GOOGLE_CLIENT_ID` are empty, the UI shows a local dev sign-in that creates a demo account. Set both variables to your Google OAuth web client ID for production sign-in.

## OpenPT / 192.168.1.174 Notes

The implementation checked `192.168.1.174` over SSH and found:

- `/home/skylarenns/Documents/GitHub/OpenPT`
- `/home/skylarenns/.config/gmbl-auth.env`
- auth env keys: `AUTH_SECRET`, `AUTH_COOKIE_NAME`, `AUTH_SESSION_DAYS`, `AUTH_ALLOWED_ORIGINS`, plus service-specific values

No Google/Gmail keys were visible in the scanned OpenPT project files. This app therefore supports the same generic auth env names and standard Google/Gmail env names without committing any secrets.

## Production Shape

- Frontend deploys to Vercel from `frontend/dist`.
- Backend deploys to the Ubuntu server as a Rust service, with Postgres and persistent artifact storage.
- Vercel must point `VITE_API_BASE_URL` to the public HTTPS backend URL.
- The backend CORS allowlist must include the Vercel production origin.

## Limits

The current geometry engine is an honest v1 heuristic engine: it validates and reads GLB geometry, uses semantic node/material names when available, then generates a measured architectural layout from scan bounds. True scan-to-wall/furniture recognition can be upgraded behind the same `processing` module with ML or point-cloud segmentation later.
