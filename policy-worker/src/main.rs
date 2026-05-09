use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use chrono::{NaiveTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_postgres::NoTls;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use wasmtime::{Engine as WasmEngine, Instance, Linker, Module, Store};

// ── types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Policy {
    id: String,
    strategy_id: String,
    mode: String,   // "alert" | "trade"
    interval: String, // "day" | "1min" | "5min" etc.
    instruments: Vec<Instrument>,
}

#[derive(Debug, Clone)]
struct Instrument {
    security_id: String,
    exchange_segment: String,
    quantity: i32,
    order_type: String,    // "MARKET" | "LIMIT"
    max_trade_value: f64,  // 0 = no limit
}

#[derive(Debug, Clone)]
struct UserCtx {
    user_id: String,
    client_id: String,
    access_token: String,
    telegram_chat_id: Option<String>,
    policies: Vec<Policy>,
}

struct WasmRunner {
    store: Store<()>,
    instance: Instance,
    /// length of closed-candle history; total alloc is history_len + 1
    history_len: usize,
    last_signal: u8,
}

impl WasmRunner {
    fn new(engine: &WasmEngine, wasm: &[u8], candles: &Candles) -> Result<Self, String> {
        let module = Module::new(engine, wasm).map_err(|e| e.to_string())?;
        let linker: Linker<()> = Linker::new(engine);
        let mut store: Store<()> = Store::new(engine, ());
        let instance = linker.instantiate(&mut store, &module).map_err(|e| e.to_string())?;

        let memory = instance.get_memory(&mut store, "memory")
            .ok_or("no memory export")?;
        let alloc_fn = instance.get_typed_func::<u32, u32>(&mut store, "alloc")
            .map_err(|e| e.to_string())?;

        // allocate history + 1 live slot
        let n = candles.closes.len();
        let total = (n + 1) as u32;
        let ptr = alloc_fn.call(&mut store, total).map_err(|e| e.to_string())?;
        {
            let data = memory.data_mut(&mut store);
            for (i, &v) in candles.closes.iter().enumerate() {
                let off = ptr as usize + i * 8;
                data[off..off + 8].copy_from_slice(&v.to_le_bytes());
            }
        }

        for (export, slice) in [
            ("alloc_volume", candles.volumes.as_slice()),
            ("alloc_high",   candles.highs.as_slice()),
            ("alloc_low",    candles.lows.as_slice()),
        ] {
            if let Ok(f) = instance.get_typed_func::<u32, u32>(&mut store, export) {
                if let Ok(ptr) = f.call(&mut store, total) {
                    let data = memory.data_mut(&mut store);
                    for (i, &v) in slice.iter().take(n).enumerate() {
                        let off = ptr as usize + i * 8;
                        data[off..off + 8].copy_from_slice(&v.to_le_bytes());
                    }
                }
            }
        }

        Ok(Self { store, instance, history_len: n, last_signal: 0 })
    }

    /// Update the live price+volume+high+low slot and run signal. Returns Some(signal) on transition.
    fn tick(&mut self, ltp: f64, vol: f64, high: f64, low: f64) -> Result<Option<u8>, String> {
        let memory = self.instance.get_memory(&mut self.store, "memory")
            .ok_or("no memory")?;
        let alloc_fn = self.instance.get_typed_func::<u32, u32>(&mut self.store, "alloc")
            .map_err(|e| e.to_string())?;

        let total = (self.history_len + 1) as u32;
        let slot = self.history_len;

        // Write LTP into last slot
        let ptr = alloc_fn.call(&mut self.store, total).map_err(|e| e.to_string())?;
        {
            let data = memory.data_mut(&mut self.store);
            let off = ptr as usize + slot * 8;
            data[off..off + 8].copy_from_slice(&ltp.to_le_bytes());
        }

        // Write live values into optional slots
        for (export, val) in [("alloc_volume", vol), ("alloc_high", high), ("alloc_low", low)] {
            if let Ok(f) = self.instance.get_typed_func::<u32, u32>(&mut self.store, export) {
                if let Ok(p) = f.call(&mut self.store, total) {
                    let data = memory.data_mut(&mut self.store);
                    let off = p as usize + slot * 8;
                    data[off..off + 8].copy_from_slice(&val.to_le_bytes());
                }
            }
        }

        let run_fn = self.instance.get_typed_func::<u32, u32>(&mut self.store, "run")
            .map_err(|e| e.to_string())?;
        let sig_ptr = run_fn.call(&mut self.store, total).map_err(|e| e.to_string())?;

        let signal = {
            let data = memory.data(&self.store);
            data[sig_ptr as usize + self.history_len]
        };

        if signal != self.last_signal {
            let prev = self.last_signal;
            self.last_signal = signal;
            if signal != 0 {
                return Ok(Some(signal));
            }
            let _ = prev;
        }
        Ok(None)
    }

    /// Run signal once over the full history (daily). Returns Some(signal) on transition.
    fn run_once(&mut self) -> Result<Option<u8>, String> {
        let total = self.history_len as u32;
        if total == 0 { return Ok(None); }

        let run_fn = self.instance.get_typed_func::<u32, u32>(&mut self.store, "run")
            .map_err(|e| e.to_string())?;
        let sig_ptr = run_fn.call(&mut self.store, total).map_err(|e| e.to_string())?;

        let memory = self.instance.get_memory(&mut self.store, "memory")
            .ok_or("no memory")?;
        let signal = {
            let data = memory.data(&self.store);
            data[sig_ptr as usize + (self.history_len - 1)]
        };

        if signal != self.last_signal && signal != 0 {
            self.last_signal = signal;
            return Ok(Some(signal));
        }
        Ok(None)
    }
}

// ── state ────────────────────────────────────────────────────────────────────

struct AppState {
    db: Arc<tokio_postgres::Client>,
    wasm_engine: WasmEngine,
    dhan_base_url: String,
    telegram_bot_token: String,
    enc_key: Vec<u8>,
    /// user_ids with intraday runners currently active
    active_intraday: Mutex<HashSet<String>>,
}

// ── db helpers ───────────────────────────────────────────────────────────────

fn decrypt_token(key: &[u8], encrypted_hex: &str) -> Result<String, String> {
    let data = hex::decode(encrypted_hex).map_err(|e| e.to_string())?;
    if data.len() < 12 {
        return Err("ciphertext too short".into());
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plain = cipher.decrypt(nonce, ciphertext).map_err(|e| e.to_string())?;
    String::from_utf8(plain).map_err(|e| e.to_string())
}

async fn load_active_policies(db: &tokio_postgres::Client, enc_key: &[u8]) -> Vec<UserCtx> {
    let rows = match db.query(
        "select u.id::text, bc.client_id, bc.encrypted_token::text, u.telegram_chat_id,
                p.id::text, p.strategy_id::text, p.mode, p.interval
         from users u
         join broker_connections bc on bc.user_id = u.id and bc.is_active = true
              and bc.token_date = (current_timestamp at time zone 'Asia/Kolkata')::date
         join policies p on p.strategy_id in (
             select id from strategies where user_id = u.id
         ) and p.status = 'active'
         order by u.id, p.id",
        &[],
    ).await {
        Ok(r) => r,
        Err(e) => { eprintln!("load_active_policies: {e}"); return vec![]; }
    };

    let mut user_map: HashMap<String, UserCtx> = HashMap::new();
    for row in &rows {
        let user_id: String = row.get(0);
        let client_id: String = row.get(1);
        let enc_token: String = row.get(2);
        let telegram_chat_id: Option<String> = row.get(3);
        let policy_id: String = row.get(4);
        let strategy_id: String = row.get(5);
        let mode: String = row.get(6);
        let interval: String = row.get(7);

        let access_token = match decrypt_token(enc_key, &enc_token) {
            Ok(t) => t,
            Err(e) => { eprintln!("decrypt token for {user_id}: {e}"); continue; }
        };

        let ctx = user_map.entry(user_id.clone()).or_insert(UserCtx {
            user_id: user_id.clone(),
            client_id: client_id.clone(),
            access_token: access_token.clone(),
            telegram_chat_id: telegram_chat_id.clone(),
            policies: vec![],
        });

        // load instruments for this policy
        let instr_rows = match db.query(
            "select security_id, exchange_segment, quantity, order_type, max_trade_value::float8
             from policy_instruments where policy_id = $1",
            &[&policy_id],
        ).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let instruments: Vec<Instrument> = instr_rows.iter().map(|r| Instrument {
            security_id: r.get::<_, &str>(0).to_string(),
            exchange_segment: r.get::<_, &str>(1).to_string(),
            quantity: r.get(2),
            order_type: r.get::<_, &str>(3).to_string(),
            max_trade_value: r.get(4),
        }).collect();

        ctx.policies.push(Policy { id: policy_id, strategy_id, mode, interval, instruments });
    }

    user_map.into_values().collect()
}

struct Candles {
    closes: Vec<f64>,
    volumes: Vec<f64>,
    highs: Vec<f64>,
    lows: Vec<f64>,
}

async fn fetch_candles(
    db: &tokio_postgres::Client,
    security_id: &str,
    exchange_segment: &str,
    interval: &str,
) -> Candles {
    let rows = db.query(
        "select close::float8, volume::float8, high::float8, low::float8 from candles
         where security_id=$1 and exchange_segment=$2 and interval=$3
         order by timestamp",
        &[&security_id, &exchange_segment, &interval],
    ).await.unwrap_or_default();
    Candles {
        closes:  rows.iter().map(|r| r.get::<_, f64>(0)).collect(),
        volumes: rows.iter().map(|r| r.get::<_, f64>(1)).collect(),
        highs:   rows.iter().map(|r| r.get::<_, f64>(2)).collect(),
        lows:    rows.iter().map(|r| r.get::<_, f64>(3)).collect(),
    }
}

async fn refresh_candles(
    db: &tokio_postgres::Client,
    dhan_base_url: &str,
    client_id: &str,
    access_token: &str,
    security_id: &str,
    exchange_segment: &str,
    interval: &str,
) {
    let today = (Utc::now() + chrono::Duration::hours(5) + chrono::Duration::minutes(30))
        .format("%Y-%m-%d").to_string();

    let max_date: Option<String> = db.query_opt(
        "select max(timestamp::date)::text from candles
         where security_id=$1 and exchange_segment=$2 and interval=$3",
        &[&security_id, &exchange_segment, &interval],
    ).await.ok().flatten().and_then(|r| r.get(0));

    let from = max_date.unwrap_or_else(|| {
        // default: last 90 days for intraday, 2 years for daily
        if interval == "day" {
            (Utc::now() - chrono::Duration::days(730)).format("%Y-%m-%d").to_string()
        } else {
            (Utc::now() - chrono::Duration::days(89)).format("%Y-%m-%d").to_string()
        }
    });

    if let Err(e) = fetch_and_store(db, dhan_base_url, client_id, access_token,
                                     security_id, exchange_segment, interval, &from, &today).await {
        eprintln!("refresh_candles {security_id}: {e}");
    }
}

// ── candle fetch (mirrors Go's candles.FetchAndStore) ────────────────────────

#[derive(Deserialize)]
struct CandleResp {
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    volume: Vec<f64>,
    timestamp: Vec<f64>,
}

fn map_segment(seg: &str) -> (&'static str, &'static str) {
    match seg {
        "NSE_E" => ("NSE_EQ", "EQUITY"),
        "BSE_E" => ("BSE_EQ", "EQUITY"),
        "NSE_I" => ("IDX_I", "INDEX"),
        _ => ("NSE_EQ", "EQUITY"),
    }
}

fn interval_minutes(interval: &str) -> u32 {
    match interval {
        "1min" => 1, "5min" => 5, "15min" => 15, "25min" => 25, "60min" => 60,
        _ => 0,
    }
}

async fn fetch_and_store(
    db: &tokio_postgres::Client,
    dhan_base_url: &str,
    client_id: &str,
    access_token: &str,
    sec_id: &str,
    seg: &str,
    interval: &str,
    from_date: &str,
    to_date: &str,
) -> Result<(), String> {
    let (dhan_seg, instr_type) = map_segment(seg);
    let mins = interval_minutes(interval);

    let chunks: Vec<(String, String)> = if mins == 0 {
        vec![(from_date.to_string(), to_date.to_string())]
    } else {
        let from = chrono::NaiveDate::parse_from_str(from_date, "%Y-%m-%d")
            .map_err(|e| e.to_string())?;
        let to = chrono::NaiveDate::parse_from_str(to_date, "%Y-%m-%d")
            .map_err(|e| e.to_string())?;
        let mut chunks = vec![];
        let mut cur = from;
        while cur <= to {
            let end = (cur + chrono::Duration::days(89)).min(to);
            chunks.push((cur.format("%Y-%m-%d").to_string(), end.format("%Y-%m-%d").to_string()));
            cur = end + chrono::Duration::days(1);
        }
        chunks
    };

    let client = reqwest::Client::new();
    for (chunk_from, chunk_to) in chunks {
        let (endpoint, payload) = if mins == 0 {
            ("/charts/historical", serde_json::json!({
                "securityId": sec_id, "exchangeSegment": dhan_seg,
                "instrument": instr_type, "expiryCode": 0,
                "fromDate": chunk_from, "toDate": chunk_to,
            }))
        } else {
            ("/charts/intraday", serde_json::json!({
                "securityId": sec_id, "exchangeSegment": dhan_seg,
                "instrument": instr_type, "interval": mins.to_string(),
                "fromDate": chunk_from, "toDate": chunk_to,
            }))
        };

        let resp = client.post(format!("{dhan_base_url}{endpoint}"))
            .header("access-token", access_token)
            .header("client-id", client_id)
            .json(&payload)
            .send().await.map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("dhan {endpoint} status {}", resp.status()));
        }

        let cr: CandleResp = resp.json().await.map_err(|e| e.to_string())?;

        let stmt = db.prepare(
            "insert into candles (security_id, exchange_segment, interval, timestamp, open, high, low, close, volume)
             values ($1, $2, $3, to_timestamp($4), $5, $6, $7, $8, $9)
             on conflict (security_id, exchange_segment, interval, timestamp)
             do update set open=excluded.open, high=excluded.high, low=excluded.low,
                           close=excluded.close, volume=excluded.volume"
        ).await.map_err(|e| e.to_string())?;

        for i in 0..cr.close.len() {
            db.execute(&stmt, &[
                &sec_id, &seg, &interval,
                &(cr.timestamp[i] as i64),
                &cr.open[i], &cr.high[i], &cr.low[i], &cr.close[i], &(cr.volume[i] as i64),
            ]).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ── signal / alert ───────────────────────────────────────────────────────────

async fn send_telegram(bot_token: &str, chat_id: &str, text: &str) {
    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
    let _ = reqwest::Client::new()
        .post(&url)
        .json(&serde_json::json!({"chat_id": chat_id, "text": text}))
        .send().await;
}

async fn record_signal(
    db: &tokio_postgres::Client,
    telegram_bot_token: &str,
    policy: &Policy,
    instrument: &Instrument,
    signal: u8,
    price: f64,
    telegram_chat_id: Option<&str>,
) {
    // signal: 1 = long (BUY), 2 = short (SELL)
    let sig_label = if signal == 1 { "LONG" } else { "SHORT" };
    let transaction_type = if signal == 1 { "BUY" } else { "SELL" };
    let product_type = if policy.interval == "day" { "CNC" } else { "INTRADAY" };

    let _ = db.execute(
        "insert into live_signals (policy_id, security_id, triggered_at, signal, price)
         values ($1, $2, now(), $3, $4)",
        &[&policy.id, &instrument.security_id, &sig_label, &price],
    ).await;

    if policy.mode == "trade" {
        // Deterministic correlation_id — prevents duplicate jobs on retry
        let correlation_id = format!(
            "{}-{}-{}-{:.0}",
            &policy.id[..8], instrument.security_id, transaction_type, price * 100.0
        );
        let _ = db.execute(
            "insert into trade_jobs
             (policy_id, security_id, exchange_segment, signal, price, quantity,
              order_type, product_type, correlation_id)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             on conflict (correlation_id) do nothing",
            &[
                &policy.id, &instrument.security_id, &instrument.exchange_segment,
                &transaction_type, &price,
                &instrument.quantity, &instrument.order_type, &product_type,
                &correlation_id,
            ],
        ).await;

        if let Some(chat_id) = telegram_chat_id {
            let text = format!(
                "⚙️ Trade queued: {} {} {} qty={} @ {:.2}",
                transaction_type, instrument.security_id, instrument.exchange_segment,
                instrument.quantity, price
            );
            send_telegram(telegram_bot_token, chat_id, &text).await;
        }
    } else if let Some(chat_id) = telegram_chat_id {
        let text = format!(
            "🔔 Signal: {} {} {} @ {:.2}",
            sig_label, instrument.security_id, instrument.exchange_segment, price
        );
        send_telegram(telegram_bot_token, chat_id, &text).await;
    }
}

// ── wasm loader ──────────────────────────────────────────────────────────────

async fn load_wasm(db: &tokio_postgres::Client, strategy_id: &str) -> Option<Vec<u8>> {
    let row = db.query_opt(
        "select wasm_key from strategies where id = $1",
        &[&strategy_id],
    ).await.ok()??;
    let wasm_key: Option<String> = row.get(0);
    let wasm_key = wasm_key?;

    // load from MinIO
    let endpoint = env::var("MINIO_ENDPOINT").unwrap_or_else(|_| "localhost:9000".to_string());
    let user = env::var("MINIO_ROOT_USER").unwrap_or_else(|_| "minioadmin".to_string());
    let pass = env::var("MINIO_ROOT_PASSWORD").unwrap_or_else(|_| "minioadmin".to_string());
    let bucket = env::var("MINIO_BUCKET").unwrap_or_else(|_| "dhan".to_string());

    let url = format!("http://{endpoint}/{bucket}/{wasm_key}");
    let resp = reqwest::Client::new()
        .get(&url)
        .basic_auth(&user, Some(&pass))
        .send().await.ok()?;
    if !resp.status().is_success() { return None; }
    Some(resp.bytes().await.ok()?.to_vec())
}

// ── market hours ─────────────────────────────────────────────────────────────

fn is_market_hours() -> bool {
    let now_ist = Utc::now() + chrono::Duration::hours(5) + chrono::Duration::minutes(30);
    let t = now_ist.time();
    t >= NaiveTime::from_hms_opt(9, 15, 0).unwrap()
        && t <= NaiveTime::from_hms_opt(15, 30, 0).unwrap()
}

// ── per-user runner ──────────────────────────────────────────────────────────

async fn run_user_policies(state: Arc<AppState>, user_ctx: UserCtx) {
    // Run daily policies immediately
    for policy in user_ctx.policies.iter().filter(|p| p.interval == "day") {
        run_daily_policy(&state, &user_ctx, policy).await;
    }

    // Spin up intraday runners only during market hours
    let intraday: Vec<&Policy> = user_ctx.policies.iter()
        .filter(|p| p.interval != "day")
        .collect();

    if intraday.is_empty() || !is_market_hours() {
        return;
    }

    run_intraday_policies(&state, &user_ctx, &intraday).await;
}

async fn run_daily_policy(state: &Arc<AppState>, user_ctx: &UserCtx, policy: &Policy) {
    let wasm = match load_wasm(&state.db, &policy.strategy_id).await {
        Some(w) => w,
        None => { eprintln!("no wasm for strategy {}", policy.strategy_id); return; }
    };

    for instrument in &policy.instruments {
        // refresh candles first
        refresh_candles(
            &state.db, &state.dhan_base_url,
            &user_ctx.client_id, &user_ctx.access_token,
            &instrument.security_id, &instrument.exchange_segment, &policy.interval,
        ).await;

        let candles = fetch_candles(
            &state.db, &instrument.security_id, &instrument.exchange_segment, &policy.interval,
        ).await;

        if candles.closes.is_empty() { continue; }

        let mut runner = match WasmRunner::new(&state.wasm_engine, &wasm, &candles) {
            Ok(r) => r,
            Err(e) => { eprintln!("wasm init: {e}"); continue; }
        };

        match runner.run_once() {
            Ok(Some(signal)) => {
                let price = candles.closes.last().copied().unwrap_or(0.0);
                record_signal(
                    &state.db, &state.telegram_bot_token,
                    policy, instrument, signal, price,
                    user_ctx.telegram_chat_id.as_deref(),
                ).await;
            }
            Ok(None) => {}
            Err(e) => eprintln!("run_once error: {e}"),
        }
    }
}

#[derive(Deserialize)]
struct DhanTick {
    ltp: f64,
    ltt: u64,
    vol: u64,
}

async fn run_intraday_policies(
    state: &Arc<AppState>,
    user_ctx: &UserCtx,
    policies: &[&Policy],
) {
    state.active_intraday.lock().await.insert(user_ctx.user_id.clone());
    // Build union of instruments across all intraday policies
    let mut instrument_set: HashMap<String, Instrument> = HashMap::new();
    for policy in policies {
        for instr in &policy.instruments {
            let key = format!("{}:{}", instr.security_id, instr.exchange_segment);
            instrument_set.entry(key).or_insert_with(|| instr.clone());
        }
    }

    // Refresh candles and build WASM runners for each (policy, instrument)
    // Map: instrument_key -> Vec<(policy_idx, WasmRunner)>
    let mut runners: HashMap<String, Vec<(usize, WasmRunner)>> = HashMap::new();

    for (pol_idx, policy) in policies.iter().enumerate() {
        let wasm = match load_wasm(&state.db, &policy.strategy_id).await {
            Some(w) => w,
            None => { eprintln!("no wasm for {}", policy.strategy_id); continue; }
        };
        for instrument in &policy.instruments {
            refresh_candles(
                &state.db, &state.dhan_base_url,
                &user_ctx.client_id, &user_ctx.access_token,
                &instrument.security_id, &instrument.exchange_segment, &policy.interval,
            ).await;

            let candles = fetch_candles(
                &state.db, &instrument.security_id, &instrument.exchange_segment, &policy.interval,
            ).await;

            if candles.closes.is_empty() { continue; }

            let runner = match WasmRunner::new(&state.wasm_engine, &wasm, &candles) {
                Ok(r) => r,
                Err(e) => { eprintln!("wasm init: {e}"); continue; }
            };

            let key = format!("{}:{}", instrument.security_id, instrument.exchange_segment);
            runners.entry(key).or_default().push((pol_idx, runner));
        }
    }

    if runners.is_empty() { return; }

    // Connect to Dhan WebSocket feed
    let dhan_feed_url = format!(
        "wss://api-feed.dhan.co?version=2&token={}&clientId={}&authType=2",
        user_ctx.access_token, user_ctx.client_id
    );

    let (ws_stream, _) = match connect_async(&dhan_feed_url).await {
        Ok(s) => s,
        Err(e) => { eprintln!("feed connect failed for {}: {e}", user_ctx.user_id); return; }
    };

    let (mut write, mut read) = ws_stream.split();

    // Subscribe to all instruments (batch into 100 per message per Dhan limit)
    let all_instrs: Vec<serde_json::Value> = instrument_set.values().map(|instr| {
        let (dhan_seg, _) = map_segment(&instr.exchange_segment);
        serde_json::json!({"ExchangeSegment": dhan_seg, "SecurityId": instr.security_id})
    }).collect();

    for chunk in all_instrs.chunks(100) {
        let sub = serde_json::json!({
            "RequestCode": 17,
            "InstrumentCount": chunk.len(),
            "InstrumentList": chunk,
        });
        if let Err(e) = write.send(Message::Text(sub.to_string())).await {
            eprintln!("feed subscribe error: {e}");
            return;
        }
    }

    // Process ticks
    while let Some(msg) = read.next().await {
        if !is_market_hours() {
            break;
        }
        let msg = match msg {
            Ok(m) => m,
            Err(e) => { eprintln!("feed read error: {e}"); break; }
        };

        // Dhan sends binary quote packets — parse security_id from packet header
        // Binary layout type=4 (50 bytes): [0]=type [1..2]=exchange_seg_code [3]=unused
        // [4..7]=security_id (u32 LE) [8..11]=LTP (f32 LE) [14..17]=LTT (u32 LE) [22..25]=vol (u32 LE)
        if let Message::Binary(b) = &msg {
            if b.len() < 50 || b[0] != 4 { continue; }

            let sec_id = u32::from_le_bytes(b[4..8].try_into().unwrap_or_default()).to_string();
            let ltp = f32::from_le_bytes(b[8..12].try_into().unwrap_or_default()) as f64;
            let vol = if b.len() >= 26 { u32::from_le_bytes(b[22..26].try_into().unwrap_or_default()) as f64 } else { 0.0 };

            // Find runners for this security_id (match by security_id prefix of key)
            for (instr_key, pol_runners) in runners.iter_mut() {
                if !instr_key.starts_with(&sec_id) { continue; }

                for (pol_idx, runner) in pol_runners.iter_mut() {
                    let policy = policies[*pol_idx];
                    let instrument = policy.instruments.iter()
                        .find(|i| i.security_id == sec_id)
                        .unwrap();

                    match runner.tick(ltp, vol, ltp, ltp) {
                        Ok(Some(signal)) => {
                            record_signal(
                                &state.db, &state.telegram_bot_token,
                                policy, instrument, signal, ltp,
                                user_ctx.telegram_chat_id.as_deref(),
                            ).await;
                        }
                        Ok(None) => {}
                        Err(e) => eprintln!("tick error: {e}"),
                    }
                }
            }
        }
    }

    // WebSocket exited (market close, disconnect, or token expiry) — deregister
    state.active_intraday.lock().await.remove(&user_ctx.user_id);
}

// ── HTTP server (user-connected endpoint) ────────────────────────────────────

async fn handle_user_connected(
    state: Arc<AppState>,
    user_id: String,
) {
    let all_users = load_active_policies(&state.db, &state.enc_key).await;
    if let Some(user_ctx) = all_users.into_iter().find(|u| u.user_id == user_id) {
        tokio::spawn(run_user_policies(state, user_ctx));
    } else {
        eprintln!("user-connected: no active policies for {user_id}");
    }
}

// ── main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL required");
    let dhan_base_url = env::var("DHAN_BASE_URL").expect("DHAN_BASE_URL required");
    let telegram_bot_token = env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default();
    let enc_key_hex = env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY required");
    let enc_key = hex::decode(&enc_key_hex).expect("ENCRYPTION_KEY must be hex");
    let port = env::var("POLICY_WORKER_PORT").unwrap_or_else(|_| "8082".to_string());

    let (db_client, connection) = tokio_postgres::connect(&db_url, NoTls).await
        .expect("db connect failed");
    tokio::spawn(async move { connection.await.expect("db connection error") });

    let state = Arc::new(AppState {
        db: Arc::new(db_client),
        wasm_engine: WasmEngine::default(),
        dhan_base_url,
        telegram_bot_token,
        enc_key,
        active_intraday: Mutex::new(HashSet::new()),
    });

    // 30s poll: re-evaluate daily policies and start intraday runners at market open
    {
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let users = load_active_policies(&state.db, &state.enc_key).await;
                let in_market = is_market_hours();
                let active = state.active_intraday.lock().await.clone();

                for user_ctx in users {
                    // Re-run daily policies every poll
                    for policy in user_ctx.policies.iter().filter(|p| p.interval == "day") {
                        run_daily_policy(&state, &user_ctx, policy).await;
                    }

                    // Start intraday runners if market is open and not already running
                    if in_market && !active.contains(&user_ctx.user_id) {
                        let intraday: Vec<&Policy> = user_ctx.policies.iter()
                            .filter(|p| p.interval != "day")
                            .collect();
                        if !intraday.is_empty() {
                            let state = state.clone();
                            let user_ctx = user_ctx.clone();
                            tokio::spawn(async move {
                                let intraday: Vec<&Policy> = user_ctx.policies.iter()
                                    .filter(|p| p.interval != "day")
                                    .collect();
                                run_intraday_policies(&state, &user_ctx, &intraday).await;
                            });
                        }
                    }
                }
            }
        });
    }

    // HTTP server
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await.expect("bind failed");
    println!("policy-worker listening on :{port}");

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let state = state.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut stream = stream;
            let mut buf = [0u8; 4096];
            let n = match stream.read(&mut buf).await {
                Ok(n) => n,
                Err(_) => return,
            };
            let req = String::from_utf8_lossy(&buf[..n]);
            // Parse: POST /internal/user-connected?user_id=...
            if let Some(line) = req.lines().next() {
                if line.starts_with("POST /internal/user-connected") {
                    if let Some(uid) = line.split("user_id=").nth(1)
                        .and_then(|s| s.split(|c: char| !c.is_alphanumeric() && c != '-').next())
                    {
                        handle_user_connected(state, uid.to_string()).await;
                    }
                }
            }
        });
    }
}
