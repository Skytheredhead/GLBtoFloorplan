# Deployment

## Vercel Frontend

Set these Vercel environment variables:

```bash
VITE_API_BASE_URL=https://floorplanapi.skylarenns.com
VITE_GOOGLE_CLIENT_ID=your-google-web-client-id.apps.googleusercontent.com
```

Deploy from the repo root. `vercel.json` builds the `frontend` workspace and serves `frontend/dist`.

## Ubuntu Backend

Build with Docker or run the compiled binary under systemd.

Required backend variables:

```bash
BIND_ADDR=0.0.0.0:8080
DATABASE_URL=postgres://...
ARTIFACT_DIR=/var/lib/glb-floorplan/artifacts
PUBLIC_BASE_URL=https://floorplanapi.skylarenns.com
FRONTEND_ORIGIN=https://floorplan.skylarenns.com
AUTH_ALLOWED_ORIGINS=https://floorplan.skylarenns.com
AUTH_SECRET=<reuse or rotate from /home/skylarenns/.config/gmbl-auth.env>
AUTH_COOKIE_NAME=glb_floorplan_session
AUTH_SESSION_DAYS=30
GOOGLE_CLIENT_ID=your-google-web-client-id.apps.googleusercontent.com
MONTHLY_FREE_SAVES=5
MAX_UPLOAD_MB=250
```

Optional Gmail variables are accepted for future account emails:

```bash
GMAIL_CLIENT_ID=
GMAIL_CLIENT_SECRET=
GMAIL_REFRESH_TOKEN=
GMAIL_SEND_FROM=
```

Run migrations before starting the service:

```bash
cd backend
sqlx migrate run
```

The public backend URL must use HTTPS for Google sign-in and Vercel browser calls.
