# GLB to Floorplan

A greenfield Vite/React frontend plus Rust `axum` backend for converting real-space `.glb` scans into measured floorplan SVG/PDF exports.

## What Works Now

- Drag/drop or file-picker upload UI.
- Anonymous Rust API for daily IP quota, GLB upload, processing progress, SVG preview, and PDF export.
- GLB/glTF parsing with transform accumulation and scale validation.
- Deterministic v1 floorplan generation from scan bounds, semantic node-name hints, measured SVG, and a vector PDF.
- 5 conversion starts per IP per UTC day. Results are held in memory only and are not stored after the backend restarts.

## Local Development

1. Copy env values:

   ```bash
   cp .env.example .env
   ```

2. Install frontend dependencies:

   ```bash
   npm install
   ```

3. Run the backend:

   ```bash
   cd backend
   PATH="$HOME/.cargo/bin:$PATH" cargo run
   ```

4. Run the frontend:

   ```bash
   npm run dev
   ```

Open `http://localhost:5173`.

## Production Shape

- Frontend deploys to Vercel from `frontend/dist`.
- Backend deploys as a stateless Rust service. No database or artifact directory is required.
- Vercel must point `VITE_API_BASE_URL` to the public HTTPS backend URL.
- The backend `ALLOWED_ORIGINS` CORS allowlist must include the Vercel production origin.

## Limits

The current geometry engine is an honest v1 heuristic engine: it validates and reads GLB geometry, uses semantic node/material names when available, then generates a measured architectural layout from scan bounds. True scan-to-wall/furniture recognition can be upgraded behind the same `processing` module with ML or point-cloud segmentation later.
