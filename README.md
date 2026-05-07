# dhan-test

Experiments with [Dhan APIs](https://dhanhq.co/docs/v2/) using Python, Rust, and Go.

## Structure

```
dhan-test/
├── python/      # Python experiments
├── rust/        # Rust experiments
└── go/          # Go experiments
```

## Setup

### Credentials

Copy `.env.example` to `.env` and fill in your Dhan API credentials:

```
DHAN_CLIENT_ID=your_client_id
DHAN_ACCESS_TOKEN=your_access_token
```

### Python

```bash
cd python
pip install -r requirements.txt
```

### Rust

```bash
cd rust
cargo build
```

### Go

```bash
cd go
go mod tidy
```

## Dhan API Docs

- Base URL: `https://api.dhan.co/v2`
- [API Reference](https://dhanhq.co/docs/v2/)
