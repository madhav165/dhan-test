# dhan-test

A multi-broker algorithmic trading platform. SvelteKit frontend, Go backend, Rust for analysis, Python for ML/stats.

## Structure

```
dhan-test/
├── web/          # SvelteKit frontend
├── go/           # Go backend service
├── rust/         # Analysis (WIP)
├── migrations/   # Postgres migrations (auto-run on first Docker start)
└── scratch/      # Throwaway scripts (not committed)
```

## Prerequisites

- Docker + Docker Compose
- Node.js (for local web dev)
- Go 1.23+ (for local Go dev)

## First-time setup

### 1. Environment

```bash
cp .env.example .env
```

Fill in:
- `POSTGRES_PASSWORD` — any password
- `DATABASE_URL` — update password to match `POSTGRES_PASSWORD`
- `SESSION_SECRET` — `openssl rand -base64 32`
- `INTERNAL_SECRET` — `openssl rand -base64 32`
- `ENCRYPTION_KEY` — `openssl rand -hex 32`
- `DHAN_APP_ID` / `DHAN_APP_SECRET` — from web.dhan.co
- `DHAN_AUTH_URL` / `DHAN_BASE_URL` — leave as default
- `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` — from console.cloud.google.com
- `GOOGLE_CALLBACK_URL` — `http://localhost:5173/auth/google/callback`

### 2. Generate SSL certs

```bash
mkdir -p certs
openssl req -new -x509 -days 365 -nodes -out certs/server.crt -keyout certs/server.key -subj "/CN=dhan-db"
chmod 600 certs/server.key
```

### 3. Install web dependencies

```bash
cd web && npm install && cd ..
```

## Running locally

| Command | Description |
|---------|-------------|
| `make up-local` | Start full local stack (Postgres, MinIO, Builder, Go) |
| `make down-local` | Stop local stack |
| `make down-clean` | Stop local stack and wipe all volumes |
| `make up` | Start Go service only (uses external DB) |
| `make down` | Stop Go service |
| `make logs` | Stream logs from all services |
| `make logs-go` | Stream logs from a specific service (`logs-builder`, etc.) |
| `make build` | Rebuild all images without cache |
| `make web` | Run SvelteKit dev server |

## Services

| Service | Port | Description |
|---------|------|-------------|
| Web (SvelteKit) | 5173 (dev) | Frontend + auth |
| Go | 8080 | Broker token storage, trading engine |
| Postgres | 5432 | Primary database (local profile only) |

## Dhan API Docs

- [API Reference](https://dhanhq.co/docs/v2/)
- Auth URL: `https://auth.dhan.co`
- Base URL: `https://api.dhan.co/v2`
