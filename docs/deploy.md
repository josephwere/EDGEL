# Deploy EDGEL (Vercel + Render)

EDGEL deploys as two services:

- EdgeStudio frontend (static) on Vercel
- GoldEdge Browser API (Rust) on Render

## Render (backend)

1. Create a new Render Web Service from this repo.
2. Select Docker runtime. Render will use `Dockerfile`.
3. Set environment variables:
   - `EDGEL_ALLOWED_ORIGIN=https://<your-vercel-domain>`
   - `NEUROEDGE_API_URL` (optional, for hosted AI)
4. Render provides `PORT` automatically.

Health check: `/health`

## Vercel (frontend)

1. Create a new Vercel project from this repo.
2. Set environment variable:
   - `EDGEL_API_BASE=https://<your-render-service>`
3. Build command: `npm run build:frontend`
4. Output directory: `dist/vercel`

## Verify

- Open your Vercel URL and ensure the Explorer sidebar loads.
- Run `Run` or `Debug` inside EdgeStudio and confirm the console and preview update.
- The backend logs should show requests from the Vercel domain.

## Notes

- CORS is enforced via `EDGEL_ALLOWED_ORIGIN`.
- The frontend reads the API base from `dist/vercel/config.js` (generated at build time).
