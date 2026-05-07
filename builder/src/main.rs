use std::env;
use std::fs;
use std::process::Command;
use std::time::Duration;
use tokio_postgres::NoTls;
use aws_sdk_s3::{Client as S3Client, config::Builder as S3Builder, config::BehaviorVersion};
use aws_credential_types::Credentials;
use aws_config::Region;
use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit}};
use base64::{Engine as B64Engine, engine::general_purpose::STANDARD as BASE64};

const SCAFFOLD: &str = r#"
use std::cell::UnsafeCell;
use indicators::rsi::rsi;
use indicators::sma::sma;
use indicators::ema::ema;

struct State { prices: Vec<f64>, signals: Vec<u8> }
struct WasmState(UnsafeCell<State>);
unsafe impl Sync for WasmState {}

static STATE: WasmState = WasmState(UnsafeCell::new(State {
    prices: Vec::new(),
    signals: Vec::new(),
}));

#[no_mangle]
pub extern "C" fn alloc(len: u32) -> *mut f64 {
    let s = unsafe { &mut *STATE.0.get() };
    s.prices = vec![0.0; len as usize];
    s.prices.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn run(len: u32) -> *mut u8 {
    let s = unsafe { &mut *STATE.0.get() };
    let n = len as usize;
    s.signals = vec![0u8; n];
    if n < 2 { return s.signals.as_mut_ptr(); }
    let prices = &s.prices[..n];
    for i in 1..n {
        s.signals[i] = signal(prices, i) as u8;
    }
    s.signals.as_mut_ptr()
}

fn signal(prices: &[f64], i: usize) -> u8 {
    // USER CODE
}
"#;

fn wrap_snippet(snippet: &str) -> String {
    SCAFFOLD.replace("// USER CODE", snippet)
}

async fn s3_client() -> S3Client {
    let endpoint = env::var("MINIO_ENDPOINT").unwrap_or_else(|_| "localhost:9000".to_string());
    let user = env::var("MINIO_ROOT_USER").unwrap_or_else(|_| "minioadmin".to_string());
    let pass = env::var("MINIO_ROOT_PASSWORD").unwrap_or_else(|_| "minioadmin".to_string());

    let creds = Credentials::new(&user, &pass, None, None, "minio");
    let config = S3Builder::new()
        .endpoint_url(format!("http://{}", endpoint))
        .credentials_provider(creds)
        .region(Region::new("us-east-1"))
        .force_path_style(true)
        .behavior_version(BehaviorVersion::latest())
        .build();

    S3Client::from_conf(config)
}

async fn upload(s3: &S3Client, bucket: &str, key: &str, data: Vec<u8>) -> Result<(), String> {
    s3.put_object()
        .bucket(bucket)
        .key(key)
        .body(data.into())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn download(s3: &S3Client, bucket: &str, key: &str) -> Result<Vec<u8>, String> {
    let resp = s3.get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let bytes = resp.body.collect().await.map_err(|e| e.to_string())?.into_bytes();
    Ok(bytes.to_vec())
}

fn decrypt_token(key_hex: &str, encoded: &str) -> Result<String, String> {
    let key_bytes = hex::decode(key_hex).map_err(|e| e.to_string())?;
    if key_bytes.len() != 32 {
        return Err("encryption key must be 32 bytes".to_string());
    }
    let ciphertext = BASE64.decode(encoded).map_err(|e| e.to_string())?;
    if ciphertext.len() < 12 {
        return Err("ciphertext too short".to_string());
    }
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&ciphertext[..12]);
    let plaintext = cipher.decrypt(nonce, &ciphertext[12..])
        .map_err(|e| e.to_string())?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

fn map_segment(seg: &str) -> (&'static str, &'static str) {
    // returns (dhan_exchange_segment, instrument_type)
    match seg {
        "NSE_E" => ("NSE_EQ", "EQUITY"),
        "BSE_E" => ("BSE_EQ", "EQUITY"),
        "NSE_I" => ("IDX_I", "INDEX"),
        _ => ("NSE_EQ", "EQUITY"),
    }
}

fn interval_minutes(interval: &str) -> Option<u32> {
    match interval {
        "1min" => Some(1),
        "5min" => Some(5),
        "15min" => Some(15),
        "25min" => Some(25),
        "60min" => Some(60),
        _ => None, // "day" → None means use historical endpoint
    }
}

#[derive(serde::Serialize)]
struct HistoricalReq {
    #[serde(rename = "securityId")]
    security_id: String,
    #[serde(rename = "exchangeSegment")]
    exchange_segment: String,
    instrument: String,
    #[serde(rename = "expiryCode")]
    expiry_code: u32,
    #[serde(rename = "fromDate")]
    from_date: String,
    #[serde(rename = "toDate")]
    to_date: String,
}

#[derive(serde::Serialize)]
struct IntradayReq {
    #[serde(rename = "securityId")]
    security_id: String,
    #[serde(rename = "exchangeSegment")]
    exchange_segment: String,
    instrument: String,
    interval: String,
    #[serde(rename = "fromDate")]
    from_date: String,
    #[serde(rename = "toDate")]
    to_date: String,
}

#[derive(serde::Deserialize, Debug)]
struct CandleResp {
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    volume: Vec<i64>,
    timestamp: Vec<i64>,
}

#[derive(serde::Serialize)]
struct CandleRecord {
    timestamp: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: i64,
    signal: u8,
}

async fn fetch_candles(
    http: &reqwest::Client,
    dhan_base: &str,
    client_id: &str,
    access_token: &str,
    security_id: &str,
    exchange_segment: &str,
    interval: &str,
    from_date: &str,
    to_date: &str,
) -> Result<CandleResp, String> {
    let (dhan_seg, instrument) = map_segment(exchange_segment);

    if let Some(mins) = interval_minutes(interval) {
        // Intraday: max 90 days per request — paginate
        let fmt = "%Y-%m-%d";
        let mut from = chrono::NaiveDate::parse_from_str(from_date, fmt).map_err(|e| e.to_string())?;
        let to = chrono::NaiveDate::parse_from_str(to_date, fmt).map_err(|e| e.to_string())?;

        let mut all = CandleResp {
            open: vec![], high: vec![], low: vec![], close: vec![],
            volume: vec![], timestamp: vec![],
        };

        while from <= to {
            let chunk_to = std::cmp::min(from + chrono::Duration::days(89), to);
            let req = IntradayReq {
                security_id: security_id.to_string(),
                exchange_segment: dhan_seg.to_string(),
                instrument: instrument.to_string(),
                interval: mins.to_string(),
                from_date: from.format(fmt).to_string(),
                to_date: chunk_to.format(fmt).to_string(),
            };

            let resp = http.post(format!("{}/charts/intraday", dhan_base))
                .header("access-token", access_token)
                .header("client-id", client_id)
                .json(&req)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            let chunk: CandleResp = resp.json().await.map_err(|e| e.to_string())?;
            all.open.extend(chunk.open);
            all.high.extend(chunk.high);
            all.low.extend(chunk.low);
            all.close.extend(chunk.close);
            all.volume.extend(chunk.volume);
            all.timestamp.extend(chunk.timestamp);

            from = chunk_to + chrono::Duration::days(1);
        }

        Ok(all)
    } else {
        // Daily
        let req = HistoricalReq {
            security_id: security_id.to_string(),
            exchange_segment: dhan_seg.to_string(),
            instrument: instrument.to_string(),
            expiry_code: 0,
            from_date: from_date.to_string(),
            to_date: to_date.to_string(),
        };

        let resp = http.post(format!("{}/charts/historical", dhan_base))
            .header("access-token", access_token)
            .header("client-id", client_id)
            .json(&req)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        resp.json().await.map_err(|e| e.to_string())
    }
}

fn run_wasm(wasm: &[u8], closes: &[f64]) -> Result<Vec<u8>, String> {
    use wasmtime::{Engine, Linker, Module, Store};

    let engine = Engine::default();
    let module = Module::new(&engine, wasm).map_err(|e| e.to_string())?;
    let linker: Linker<()> = Linker::new(&engine);
    let mut store: Store<()> = Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &module).map_err(|e| e.to_string())?;

    let memory = instance.get_memory(&mut store, "memory")
        .ok_or_else(|| "no memory export".to_string())?;
    let alloc_fn = instance.get_typed_func::<u32, u32>(&mut store, "alloc")
        .map_err(|e| e.to_string())?;
    let run_fn = instance.get_typed_func::<u32, u32>(&mut store, "run")
        .map_err(|e| e.to_string())?;

    let len = closes.len() as u32;
    let ptr = alloc_fn.call(&mut store, len).map_err(|e| e.to_string())?;

    {
        let data = memory.data_mut(&mut store);
        for (i, &v) in closes.iter().enumerate() {
            let off = ptr as usize + i * 8;
            data[off..off + 8].copy_from_slice(&v.to_le_bytes());
        }
    }

    let sig_ptr = run_fn.call(&mut store, len).map_err(|e| e.to_string())?;

    let signals = {
        let data = memory.data(&store);
        data[sig_ptr as usize..sig_ptr as usize + len as usize].to_vec()
    };

    Ok(signals)
}

struct Metrics {
    num_trades: i32,
    total_pnl: f64,
    win_rate: f64,
    max_drawdown: f64,
}

fn compute_metrics(closes: &[f64], signals: &[u8]) -> Metrics {
    let mut num_trades = 0i32;
    let mut wins = 0i32;
    let mut total_pnl = 0.0f64;
    let mut entry: Option<f64> = None;
    let mut cum_pnl = 0.0f64;
    let mut peak = 0.0f64;
    let mut max_drawdown = 0.0f64;

    for (i, &sig) in signals.iter().enumerate() {
        match sig {
            1 if entry.is_none() => {
                entry = Some(closes[i]);
            }
            2 if entry.is_some() => {
                let e = entry.take().unwrap();
                let pnl = closes[i] - e;
                total_pnl += pnl;
                num_trades += 1;
                if pnl > 0.0 { wins += 1; }
                cum_pnl += pnl;
                if cum_pnl > peak { peak = cum_pnl; }
                let dd = peak - cum_pnl;
                if dd > max_drawdown { max_drawdown = dd; }
            }
            _ => {}
        }
    }

    // Close open position at last bar
    if let Some(e) = entry {
        if let Some(&last) = closes.last() {
            let pnl = last - e;
            total_pnl += pnl;
            num_trades += 1;
            if pnl > 0.0 { wins += 1; }
        }
    }

    let win_rate = if num_trades > 0 { wins as f64 / num_trades as f64 } else { 0.0 };
    Metrics { num_trades, total_pnl, win_rate, max_drawdown }
}

async fn process_run_job(
    db: &tokio_postgres::Client,
    s3: &S3Client,
    bucket: &str,
    http: &reqwest::Client,
    dhan_base: &str,
    encryption_key: &str,
    job_id: uuid::Uuid,
    run_id: uuid::Uuid,
) -> Result<(), String> {
    db.execute(
        "update run_jobs set status='running', updated_at=now() where id=$1",
        &[&job_id],
    ).await.map_err(|e| e.to_string())?;

    // Fetch run details
    let run_row = db.query_one(
        "select r.interval, r.from_date::text, r.to_date::text, \
                s.wasm_key, s.user_id \
         from backtest_runs r \
         join strategies s on s.id = r.strategy_id \
         where r.id = $1",
        &[&run_id],
    ).await.map_err(|e| e.to_string())?;

    let interval: String = run_row.get(0);
    let from_date: String = run_row.get(1);
    let to_date: String = run_row.get(2);
    let wasm_key: Option<String> = run_row.get(3);
    let user_id: uuid::Uuid = run_row.get(4);

    let wasm_key = wasm_key.ok_or_else(|| "strategy not compiled".to_string())?;

    // Fetch instruments
    let inst_rows = db.query(
        "select security_id, exchange_segment from backtest_run_instruments where run_id = $1",
        &[&run_id],
    ).await.map_err(|e| e.to_string())?;

    if inst_rows.is_empty() {
        return Err("no instruments for run".to_string());
    }

    // Get user's Dhan token
    let token_row = db.query_one(
        "select client_id, encrypted_token from broker_connections \
         where user_id = $1 and broker = 'dhan' \
         and token_date = (current_timestamp at time zone 'Asia/Kolkata')::date \
         and is_active = true",
        &[&user_id],
    ).await.map_err(|_| "no active dhan token".to_string())?;

    let client_id: String = token_row.get(0);
    let encrypted_token: String = token_row.get(1);
    let access_token = decrypt_token(encryption_key, &encrypted_token)?;

    // Download WASM
    let wasm = download(s3, bucket, &wasm_key).await?;

    // Run WASM for each instrument and collect metrics
    let mut all_records: Vec<serde_json::Value> = vec![];
    let mut total_trades = 0i32;
    let mut total_pnl = 0.0f64;
    let mut total_wins = 0i32;
    let mut max_drawdown = 0.0f64;

    for row in &inst_rows {
        let security_id: &str = row.get(0);
        let exchange_segment: &str = row.get(1);

        let candles = fetch_candles(
            http, dhan_base, &client_id, &access_token,
            security_id, exchange_segment, &interval, &from_date, &to_date,
        ).await?;

        if candles.close.is_empty() {
            continue;
        }

        let signals = run_wasm(&wasm, &candles.close)?;
        let m = compute_metrics(&candles.close, &signals);

        total_trades += m.num_trades;
        total_pnl += m.total_pnl;
        total_wins += (m.win_rate * m.num_trades as f64).round() as i32;
        if m.max_drawdown > max_drawdown { max_drawdown = m.max_drawdown; }

        let records: Vec<CandleRecord> = (0..candles.close.len())
            .map(|i| CandleRecord {
                timestamp: *candles.timestamp.get(i).unwrap_or(&0),
                open: *candles.open.get(i).unwrap_or(&0.0),
                high: *candles.high.get(i).unwrap_or(&0.0),
                low: *candles.low.get(i).unwrap_or(&0.0),
                close: candles.close[i],
                volume: *candles.volume.get(i).unwrap_or(&0),
                signal: signals[i],
            })
            .collect();

        all_records.push(serde_json::json!({
            "security_id": security_id,
            "exchange_segment": exchange_segment,
            "candles": records,
        }));
    }

    let win_rate = if total_trades > 0 { total_wins as f64 / total_trades as f64 } else { 0.0 };

    // Store result JSON in MinIO
    let result_json = serde_json::to_vec(&all_records).map_err(|e| e.to_string())?;
    let result_key = format!("runs/{}/result.json", run_id);
    upload(s3, bucket, &result_key, result_json).await?;

    // Update backtest_runs
    db.execute(
        "update backtest_runs set result_key=$1, num_trades=$2, \
         total_pnl=$3, win_rate=$4, max_drawdown=$5 where id=$6",
        &[&result_key, &total_trades, &total_pnl, &win_rate, &max_drawdown, &run_id],
    ).await.map_err(|e| e.to_string())?;

    Ok(())
}

async fn process_job(
    db: &tokio_postgres::Client,
    s3: &S3Client,
    bucket: &str,
    job_id: &str,
    strategy_id: &str,
    snippet: &str,
) -> Result<(String, String), String> {
    // Mark as building
    db.execute(
        "update build_jobs set status='building', updated_at=now() where id=$1",
        &[&uuid::Uuid::parse_str(job_id).unwrap()],
    ).await.map_err(|e| e.to_string())?;

    // Create temp workspace
    let dir = format!("/tmp/strategy_{}", job_id);
    fs::create_dir_all(format!("{}/src", dir)).map_err(|e| e.to_string())?;

    // Write Cargo.toml
    fs::write(format!("{}/Cargo.toml", dir), format!(r#"
[package]
name = "strategy"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
indicators = {{ path = "/app/indicators" }}
"#)).map_err(|e| e.to_string())?;

    // Write lib.rs with scaffold + user snippet
    let source = wrap_snippet(snippet);
    fs::write(format!("{}/src/lib.rs", dir), &source).map_err(|e| e.to_string())?;

    // Compile
    let output = Command::new("cargo")
        .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
        .current_dir(&dir)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        fs::remove_dir_all(&dir).ok();
        return Err(err);
    }

    // Upload source
    let source_key = format!("strategies/{}/source.rs", strategy_id);
    upload(s3, bucket, &source_key, source.into_bytes()).await?;

    // Upload WASM
    let wasm_path = format!("{}/target/wasm32-unknown-unknown/release/strategy.wasm", dir);
    let wasm = fs::read(&wasm_path).map_err(|e| e.to_string())?;
    let wasm_key = format!("strategies/{}/strategy.wasm", strategy_id);
    upload(s3, bucket, &wasm_key, wasm).await?;

    fs::remove_dir_all(&dir).ok();
    Ok((source_key, wasm_key))
}

#[tokio::main]
async fn main() {
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL required");
    let bucket = env::var("MINIO_BUCKET").unwrap_or_else(|_| "dhan".to_string());
    let dhan_base = env::var("DHAN_BASE_URL").unwrap_or_else(|_| "https://api.dhan.co".to_string());
    let encryption_key = env::var("ENCRYPTION_KEY").unwrap_or_default();

    let (db, connection) = tokio_postgres::connect(&db_url, NoTls).await.expect("db connect failed");
    tokio::spawn(async move { connection.await.expect("db connection error") });

    let s3 = s3_client().await;
    let http = reqwest::Client::new();

    // Ensure bucket exists
    s3.create_bucket().bucket(&bucket).send().await.ok();

    println!("builder: polling for jobs");

    loop {
        // --- build jobs ---
        let rows = db.query(
            "select j.id, j.strategy_id, s.source_key
             from build_jobs j
             join strategies s on s.id = j.strategy_id
             where j.status = 'pending'
             order by j.created_at
             limit 1",
            &[],
        ).await.unwrap_or_default();

        for row in rows {
            let job_id: uuid::Uuid = row.get(0);
            let strategy_id: uuid::Uuid = row.get(1);
            let snippet: Option<String> = row.get(2);

            let snippet = match snippet {
                Some(s) => s,
                None => {
                    db.execute(
                        "update build_jobs set status='failed', error='no source code', updated_at=now() where id=$1",
                        &[&job_id],
                    ).await.ok();
                    continue;
                }
            };

            println!("builder: compiling strategy {}", strategy_id);

            match process_job(&db, &s3, &bucket, &job_id.to_string(), &strategy_id.to_string(), &snippet).await {
                Ok((source_key, wasm_key)) => {
                    db.execute(
                        "update build_jobs set status='done', updated_at=now() where id=$1",
                        &[&job_id],
                    ).await.ok();
                    db.execute(
                        "update strategies set source_key=$1, wasm_key=$2 where id=$3",
                        &[&source_key, &wasm_key, &strategy_id],
                    ).await.ok();
                    println!("builder: done {}", strategy_id);
                }
                Err(e) => {
                    eprintln!("builder: failed {}: {}", strategy_id, e);
                    db.execute(
                        "update build_jobs set status='failed', error=$1, updated_at=now() where id=$2",
                        &[&e, &job_id],
                    ).await.ok();
                }
            }
        }

        // --- run jobs ---
        let run_rows = db.query(
            "select id, run_id from run_jobs where status = 'pending' order by created_at limit 1",
            &[],
        ).await.unwrap_or_default();

        for row in run_rows {
            let job_id: uuid::Uuid = row.get(0);
            let run_id: uuid::Uuid = row.get(1);

            println!("builder: running backtest {}", run_id);

            match process_run_job(&db, &s3, &bucket, &http, &dhan_base, &encryption_key, job_id, run_id).await {
                Ok(()) => {
                    db.execute(
                        "update run_jobs set status='done', updated_at=now() where id=$1",
                        &[&job_id],
                    ).await.ok();
                    println!("builder: run done {}", run_id);
                }
                Err(e) => {
                    eprintln!("builder: run failed {}: {}", run_id, e);
                    db.execute(
                        "update run_jobs set status='failed', error=$1, updated_at=now() where id=$2",
                        &[&e, &job_id],
                    ).await.ok();
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}
