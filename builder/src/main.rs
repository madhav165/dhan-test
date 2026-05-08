use std::env;
use std::fs;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use rayon::prelude::*;
use tokio_postgres::NoTls;
use aws_sdk_s3::{Client as S3Client, config::Builder as S3Builder, config::BehaviorVersion};
use aws_credential_types::Credentials;
use aws_config::Region;
use arrow::array::{Float64Array, Int64Array, StringArray, UInt8Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

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

#[derive(Clone)]
struct BrokerCharges {
    brokerage_flat: f64,
    brokerage_pct: f64,
    stt_buy_pct: f64,
    stt_sell_pct: f64,
    exchange_pct: f64,
    sebi_pct: f64,
    stamp_buy_pct: f64,
    gst_pct: f64,
}

impl BrokerCharges {
    fn cost(&self, buy_price: f64, sell_price: f64) -> f64 {
        let brokerage = (self.brokerage_pct * buy_price).min(self.brokerage_flat)
            + (self.brokerage_pct * sell_price).min(self.brokerage_flat);
        let stt = self.stt_buy_pct * buy_price + self.stt_sell_pct * sell_price;
        let exchange = self.exchange_pct * (buy_price + sell_price);
        let sebi = self.sebi_pct * (buy_price + sell_price);
        let stamp = self.stamp_buy_pct * buy_price;
        let gst = self.gst_pct * (brokerage + exchange + sebi);
        brokerage + stt + exchange + sebi + stamp + gst
    }
}

fn compute_metrics(closes: &[f64], signals: &[u8], charges: &BrokerCharges) -> Metrics {
    let mut num_trades = 0i32;
    let mut wins = 0i32;
    let mut total_pnl = 0.0f64;
    // (entry_price, is_long)
    let mut entry: Option<(f64, bool)> = None;
    let mut cum_pnl = 0.0f64;
    let mut peak = 0.0f64;
    let mut max_drawdown = 0.0f64;

    let mut record = |gross_pnl: f64, buy_price: f64, sell_price: f64| {
        let cost = charges.cost(buy_price, sell_price);
        let pnl = gross_pnl - cost;
        total_pnl += pnl;
        num_trades += 1;
        if pnl > 0.0 { wins += 1; }
        cum_pnl += pnl;
        if cum_pnl > peak { peak = cum_pnl; }
        let dd = peak - cum_pnl;
        if dd > max_drawdown { max_drawdown = dd; }
    };

    for (i, &sig) in signals.iter().enumerate() {
        let is_long = sig == 1;
        if sig == 0 { continue; }
        if let Some((e, prev_long)) = entry.take() {
            let (gross, buy_p, sell_p) = if prev_long {
                (closes[i] - e, e, closes[i])
            } else {
                (e - closes[i], closes[i], e)
            };
            record(gross, buy_p, sell_p);
        }
        entry = Some((closes[i], is_long));
    }

    if let Some((e, is_long)) = entry {
        if let Some(&last) = closes.last() {
            let (gross, buy_p, sell_p) = if is_long {
                (last - e, e, last)
            } else {
                (e - last, last, e)
            };
            record(gross, buy_p, sell_p);
        }
    }

    let win_rate = if num_trades > 0 { wins as f64 / num_trades as f64 } else { 0.0 };
    Metrics { num_trades, total_pnl, win_rate, max_drawdown }
}

fn build_parquet(
    security_ids: &[String],
    exchange_segments: &[String],
    timestamps: &[i64],
    opens: &[f64],
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    volumes: &[i64],
    signals: &[u8],
) -> Result<Vec<u8>, String> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("security_id", DataType::Utf8, false),
        Field::new("exchange_segment", DataType::Utf8, false),
        Field::new("timestamp", DataType::Int64, false),
        Field::new("open", DataType::Float64, false),
        Field::new("high", DataType::Float64, false),
        Field::new("low", DataType::Float64, false),
        Field::new("close", DataType::Float64, false),
        Field::new("volume", DataType::Int64, false),
        Field::new("signal", DataType::UInt8, false),
    ]));

    let batch = RecordBatch::try_new(schema.clone(), vec![
        Arc::new(StringArray::from(security_ids.to_vec())),
        Arc::new(StringArray::from(exchange_segments.to_vec())),
        Arc::new(Int64Array::from(timestamps.to_vec())),
        Arc::new(Float64Array::from(opens.to_vec())),
        Arc::new(Float64Array::from(highs.to_vec())),
        Arc::new(Float64Array::from(lows.to_vec())),
        Arc::new(Float64Array::from(closes.to_vec())),
        Arc::new(Int64Array::from(volumes.to_vec())),
        Arc::new(UInt8Array::from(signals.to_vec())),
    ]).map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    let mut writer = ArrowWriter::try_new(&mut buf, schema, None).map_err(|e| e.to_string())?;
    writer.write(&batch).map_err(|e| e.to_string())?;
    writer.close().map_err(|e| e.to_string())?;
    Ok(buf)
}

async fn process_run_job(
    db: &tokio_postgres::Client,
    s3: &S3Client,
    bucket: &str,
    job_id: uuid::Uuid,
    run_id: uuid::Uuid,
) -> Result<(), String> {
    db.execute(
        "update run_jobs set status='running', updated_at=now() where id=$1",
        &[&job_id],
    ).await.map_err(|e| e.to_string())?;

    let run_row = db.query_one(
        "select r.interval, r.from_date::text, r.to_date::text, s.wasm_key \
         from backtest_runs r \
         join strategies s on s.id = r.strategy_id \
         where r.id = $1",
        &[&run_id],
    ).await.map_err(|e| e.to_string())?;

    let interval: String = run_row.get(0);
    let from_date: String = run_row.get(1);
    let to_date: String = run_row.get(2);
    let wasm_key: Option<String> = run_row.get(3);
    let wasm_key = wasm_key.ok_or_else(|| "strategy not compiled".to_string())?;

    let inst_rows = db.query(
        "select security_id, exchange_segment from backtest_run_instruments where run_id = $1",
        &[&run_id],
    ).await.map_err(|e| e.to_string())?;

    if inst_rows.is_empty() {
        return Err("no instruments for run".to_string());
    }

    let wasm = download(s3, bucket, &wasm_key).await?;

    // Fetch all candle data async first
    struct InstCandles {
        security_id: String,
        exchange_segment: String,
        timestamps: Vec<i64>,
        opens: Vec<f64>,
        highs: Vec<f64>,
        lows: Vec<f64>,
        closes: Vec<f64>,
        volumes: Vec<i64>,
    }

    let mut inst_candles: Vec<InstCandles> = vec![];
    for row in &inst_rows {
        let security_id: String = row.get::<_, &str>(0).to_string();
        let exchange_segment: String = row.get::<_, &str>(1).to_string();

        let candle_rows = db.query(
            "select extract(epoch from timestamp)::bigint, open::float8, high::float8, \
                    low::float8, close::float8, volume \
             from candles \
             where security_id=$1 and exchange_segment=$2 and interval=$3 \
             and timestamp::date between $4::text::date and $5::text::date \
             order by timestamp",
            &[&security_id.as_str(), &exchange_segment.as_str(), &interval, &from_date, &to_date],
        ).await.map_err(|e| e.to_string())?;

        if candle_rows.is_empty() { continue; }

        let mut ic = InstCandles {
            security_id, exchange_segment,
            timestamps: vec![], opens: vec![], highs: vec![],
            lows: vec![], closes: vec![], volumes: vec![],
        };
        for r in &candle_rows {
            ic.timestamps.push(r.get(0));
            ic.opens.push(r.get(1));
            ic.highs.push(r.get(2));
            ic.lows.push(r.get(3));
            ic.closes.push(r.get(4));
            ic.volumes.push(r.get(5));
        }
        inst_candles.push(ic);
    }

    if inst_candles.is_empty() {
        return Err("no candles found in DB for any instrument".to_string());
    }

    let trade_type = if interval == "day" { "delivery" } else { "intraday" };
    let charge_row = db.query_one(
        "select brokerage_flat::float8, brokerage_pct::float8, stt_buy_pct::float8, \
                stt_sell_pct::float8, exchange_pct::float8, sebi_pct::float8, \
                stamp_buy_pct::float8, gst_pct::float8 \
         from broker_charges where trade_type = $1",
        &[&trade_type],
    ).await.map_err(|e| e.to_string())?;
    let charges = BrokerCharges {
        brokerage_flat: charge_row.get(0),
        brokerage_pct:  charge_row.get(1),
        stt_buy_pct:    charge_row.get(2),
        stt_sell_pct:   charge_row.get(3),
        exchange_pct:   charge_row.get(4),
        sebi_pct:       charge_row.get(5),
        stamp_buy_pct:  charge_row.get(6),
        gst_pct:        charge_row.get(7),
    };

    // Parallel WASM execution across instruments
    struct InstResult {
        security_id: String,
        exchange_segment: String,
        timestamps: Vec<i64>,
        opens: Vec<f64>,
        highs: Vec<f64>,
        lows: Vec<f64>,
        closes: Vec<f64>,
        volumes: Vec<i64>,
        signals: Vec<u8>,
        metrics: Metrics,
    }

    let results: Result<Vec<InstResult>, String> = inst_candles
        .into_par_iter()
        .map(|ic| {
            let signals = run_wasm(&wasm, &ic.closes)?;
            let metrics = compute_metrics(&ic.closes, &signals, &charges.clone());
            Ok(InstResult {
                security_id: ic.security_id,
                exchange_segment: ic.exchange_segment,
                timestamps: ic.timestamps,
                opens: ic.opens,
                highs: ic.highs,
                lows: ic.lows,
                closes: ic.closes,
                volumes: ic.volumes,
                signals,
                metrics,
            })
        })
        .collect();

    let results = results?;

    // Aggregate
    let mut col_sec: Vec<String> = vec![];
    let mut col_seg: Vec<String> = vec![];
    let mut col_ts: Vec<i64> = vec![];
    let mut col_open: Vec<f64> = vec![];
    let mut col_high: Vec<f64> = vec![];
    let mut col_low: Vec<f64> = vec![];
    let mut col_close: Vec<f64> = vec![];
    let mut col_vol: Vec<i64> = vec![];
    let mut col_sig: Vec<u8> = vec![];
    let mut total_trades = 0i32;
    let mut total_wins = 0i32;
    let mut total_pnl = 0.0f64;
    let mut max_drawdown = 0.0f64;
    let mut sig_map: std::collections::BTreeMap<String, Vec<serde_json::Value>> = std::collections::BTreeMap::new();

    for r in results {
        let n = r.closes.len();
        total_trades += r.metrics.num_trades;
        total_wins += (r.metrics.win_rate * r.metrics.num_trades as f64).round() as i32;
        total_pnl += r.metrics.total_pnl;
        if r.metrics.max_drawdown > max_drawdown { max_drawdown = r.metrics.max_drawdown; }

        let key = format!("{}:{}", r.security_id, r.exchange_segment);
        let entries: Vec<serde_json::Value> = r.timestamps.iter().zip(r.signals.iter())
            .filter(|(_, &sig)| sig != 0)
            .map(|(&ts, &sig)| serde_json::json!({"ts": ts, "sig": sig}))
            .collect();
        if !entries.is_empty() {
            sig_map.insert(key, entries);
        }

        col_sec.extend(std::iter::repeat(r.security_id).take(n));
        col_seg.extend(std::iter::repeat(r.exchange_segment).take(n));
        col_ts.extend_from_slice(&r.timestamps);
        col_open.extend_from_slice(&r.opens);
        col_high.extend_from_slice(&r.highs);
        col_low.extend_from_slice(&r.lows);
        col_close.extend_from_slice(&r.closes);
        col_vol.extend_from_slice(&r.volumes);
        col_sig.extend_from_slice(&r.signals);
    }

    let win_rate = if total_trades > 0 { total_wins as f64 / total_trades as f64 } else { 0.0 };

    let parquet = build_parquet(
        &col_sec, &col_seg, &col_ts,
        &col_open, &col_high, &col_low, &col_close, &col_vol, &col_sig,
    )?;

    let result_key = format!("runs/{}/result.parquet", run_id);
    upload(s3, bucket, &result_key, parquet).await?;

    let signals_json = serde_json::to_vec(&sig_map).map_err(|e| e.to_string())?;
    upload(s3, bucket, &format!("runs/{}/signals.json", run_id), signals_json).await?;

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
    db.execute(
        "update build_jobs set status='building', updated_at=now() where id=$1",
        &[&uuid::Uuid::parse_str(job_id).unwrap()],
    ).await.map_err(|e| e.to_string())?;

    let dir = format!("/tmp/strategy_{}", job_id);
    fs::create_dir_all(format!("{}/src", dir)).map_err(|e| e.to_string())?;

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

    let source = wrap_snippet(snippet);
    fs::write(format!("{}/src/lib.rs", dir), &source).map_err(|e| e.to_string())?;

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

    let source_key = format!("strategies/{}/source.rs", strategy_id);
    upload(s3, bucket, &source_key, source.into_bytes()).await?;

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

    let (db, connection) = tokio_postgres::connect(&db_url, NoTls).await.expect("db connect failed");
    tokio::spawn(async move { connection.await.expect("db connection error") });

    let s3 = s3_client().await;
    s3.create_bucket().bucket(&bucket).send().await.ok();

    println!("builder: polling for jobs");

    loop {
        // build jobs
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

        // run jobs — only process ones marked ready by Go worker
        let run_rows = db.query(
            "select id, run_id from run_jobs where status = 'ready' order by created_at limit 1",
            &[],
        ).await.unwrap_or_default();

        for row in run_rows {
            let job_id: uuid::Uuid = row.get(0);
            let run_id: uuid::Uuid = row.get(1);

            println!("builder: executing run {}", run_id);

            match process_run_job(&db, &s3, &bucket, job_id, run_id).await {
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
