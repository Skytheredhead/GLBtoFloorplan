# Deployment

## Vercel Frontend

Set these Vercel environment variables:

```bash
VITE_API_BASE_URL=https://floorplanapi.skylarenns.com
```

Deploy from the repo root. `vercel.json` builds the `frontend` workspace and serves `frontend/dist`.

## Ubuntu Backend

Build with Docker or run the compiled binary under systemd.

Required backend variables:

```bash
BIND_ADDR=0.0.0.0:8080
PUBLIC_BASE_URL=https://floorplanapi.skylarenns.com
FRONTEND_ORIGIN=https://floorplan.skylarenns.com
ALLOWED_ORIGINS=https://floorplan.skylarenns.com
DAILY_IP_CONVERTS=5
MAX_UPLOAD_MB=250
```

The backend keeps quota counters and conversion results in memory only. Restarting the service clears current results and daily counters.
