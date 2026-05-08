COMPOSE = docker compose
LOCAL   = $(COMPOSE) --profile local

.PHONY: help up up-local down down-local down-clean logs build wasm web migrate

## Show this help
help:
	@awk '/^## /{desc=substr($$0,4); next} /^[a-zA-Z%_-]+:/{if (desc) printf "%-15s %s\n", substr($$1,1,length($$1)-1), desc; desc=""}' Makefile

## Start Go service only (needs external DB)
up:
	$(COMPOSE) up --build -d

## Start full local stack (Postgres + MinIO + Builder + Go)
up-local: wasm
	$(LOCAL) up --build -d

## Stop Go service
down:
	$(COMPOSE) down

## Stop local stack
down-local:
	$(LOCAL) down

## Stop local stack and wipe volumes (DB + MinIO data)
down-clean:
	$(LOCAL) down -v

## Stream logs (all services)
logs:
	$(LOCAL) logs -f

## Stream logs for a specific service: make logs-go, make logs-builder
logs-%:
	$(LOCAL) logs -f $*

## Rebuild without cache
build:
	$(LOCAL) build --no-cache

wasm:
	cd rust && cargo build -p indicators-wasm --target wasm32-unknown-unknown --release
	cp rust/target/wasm32-unknown-unknown/release/indicators_wasm.wasm web/static/indicators.wasm

## Run pending migrations (reads DATABASE_URL from .env)
migrate:
	@set -a && . ./.env && set +a && migrate -path migrations -database "$$DATABASE_URL" up

## Run web dev server
web:
	cd web && npm run dev

