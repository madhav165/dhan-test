mod rl;
use rl::features::{Candles, IndicatorSpec, compute_indicators, build_state_matrix_with_indices, normalise_with_stats, apply_normalisation_stats};
use rl::train::{TrainConfig, train_reinforce, train_ppo, weights_to_bytes, split_points, collect_greedy_states};
use rl::distill::{feature_importance, normalise_importance, distil, net_to_rust};

use std::env;
use std::fs;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use tokio_postgres::NoTls;
use aws_sdk_s3::{Client as S3Client, config::Builder as S3Builder, config::BehaviorVersion};
use aws_credential_types::Credentials;
use aws_config::Region;
use arrow::array::{Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

const SCAFFOLD: &str = r#"
use std::cell::UnsafeCell;
use indicators::rsi::rsi;
use indicators::sma::sma;
use indicators::ema::ema;
use indicators::wma::wma;
use indicators::macd::macd;
use indicators::bb::bb;
use indicators::vwap::vwap;
use indicators::atr::atr;
use indicators::stoch::stoch;
use indicators::obv::obv;
use indicators::cci::cci;

struct State {
    prices: Vec<f64>,
    opens: Vec<f64>,
    volumes: Vec<f64>,
    highs: Vec<f64>,
    lows: Vec<f64>,
    signals: Vec<f64>,
}
struct WasmState(UnsafeCell<State>);
unsafe impl Sync for WasmState {}

static STATE: WasmState = WasmState(UnsafeCell::new(State {
    prices: Vec::new(),
    opens: Vec::new(),
    volumes: Vec::new(),
    highs: Vec::new(),
    lows: Vec::new(),
    signals: Vec::new(),
}));

#[no_mangle]
pub extern "C" fn alloc(len: u32) -> *mut f64 {
    let s = unsafe { &mut *STATE.0.get() };
    s.prices = vec![0.0; len as usize];
    s.prices.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn alloc_open(len: u32) -> *mut f64 {
    let s = unsafe { &mut *STATE.0.get() };
    s.opens = vec![0.0; len as usize];
    s.opens.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn alloc_volume(len: u32) -> *mut f64 {
    let s = unsafe { &mut *STATE.0.get() };
    s.volumes = vec![0.0; len as usize];
    s.volumes.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn alloc_high(len: u32) -> *mut f64 {
    let s = unsafe { &mut *STATE.0.get() };
    s.highs = vec![0.0; len as usize];
    s.highs.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn alloc_low(len: u32) -> *mut f64 {
    let s = unsafe { &mut *STATE.0.get() };
    s.lows = vec![0.0; len as usize];
    s.lows.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn run(len: u32) -> *mut f64 {
    let s = unsafe { &mut *STATE.0.get() };
    let n = len as usize;
    s.signals = vec![0.0; n];
    if n < 2 { return s.signals.as_mut_ptr(); }
    let prices = &s.prices[..n];
    let opens = &s.opens[..s.opens.len().min(n)];
    let volumes = &s.volumes[..s.volumes.len().min(n)];
    let highs = &s.highs[..s.highs.len().min(n)];
    let lows = &s.lows[..s.lows.len().min(n)];
    let mut position: f64 = 0.0;
    let mut entry_price = 0.0_f64;
    let mut holding = 0_usize;
    let mut prev_target: f64 = 0.0;
    for i in 1..n {
        let price = prices[i];
        let unrealized_pnl = if position > 0.0 {
            position * (price - entry_price)
        } else if position < 0.0 {
            position.abs() * (entry_price - price)
        } else {
            0.0
        };
        let norm_position = position;
        let norm_holding = (holding as f64 / 20.0).clamp(-5.0, 5.0);
        let norm_unrealized = (unrealized_pnl / entry_price.max(1e-8)).clamp(-5.0, 5.0);
        let target = signal(prices, opens, volumes, highs, lows, i, norm_position, norm_holding, norm_unrealized);
        let target = if target.is_nan() { prev_target } else { target.clamp(-1.0, 1.0) };
        s.signals[i] = target;

        if target != position {
            if position > 0.0 && target < position {
                let closed = position - target.max(0.0);
                let _ = closed * (price - entry_price);
            } else if position < 0.0 && target > position {
                let closed = position.abs() - target.min(0.0).abs();
                let _ = closed * (entry_price - price);
            }
            if target == 0.0 {
                entry_price = 0.0;
            } else if position == 0.0 {
                entry_price = price;
            } else if position.signum() != target.signum() {
                entry_price = price;
            } else {
                let same_dir = if position > 0.0 { position.min(target) } else { position.max(target) };
                let added = (target - position).abs();
                let new_size = target.abs();
                if new_size > 0.0 {
                    entry_price = (entry_price * same_dir.abs() + price * added) / new_size;
                }
            }
        }
        position = target;
        holding = if position != 0.0 { holding + 1 } else { 0 };
        prev_target = target;
    }
    s.signals.as_mut_ptr()
}

fn signal(prices: &[f64], opens: &[f64], volumes: &[f64], highs: &[f64], lows: &[f64], i: usize, norm_position: f64, norm_holding: f64, norm_unrealized: f64) -> f64 {
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

fn run_wasm(wasm: &[u8], closes: &[f64], opens: &[f64], volumes: &[f64], highs: &[f64], lows: &[f64]) -> Result<Vec<f64>, String> {
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

    for (export, slice) in [("alloc_open", opens), ("alloc_volume", volumes), ("alloc_high", highs), ("alloc_low", lows)] {
        if let Ok(f) = instance.get_typed_func::<u32, u32>(&mut store, export) {
            let ptr = f.call(&mut store, len).map_err(|e| e.to_string())?;
            let data = memory.data_mut(&mut store);
            for (i, &v) in slice.iter().take(len as usize).enumerate() {
                let off = ptr as usize + i * 8;
                data[off..off + 8].copy_from_slice(&v.to_le_bytes());
            }
        }
    }

    let sig_ptr = run_fn.call(&mut store, len).map_err(|e| e.to_string())?;

    let signals: Vec<f64> = {
        let data = memory.data(&store);
        (0..len as usize)
            .map(|i| {
                let off = sig_ptr as usize + i * 8;
                f64::from_le_bytes([
                    data[off], data[off + 1], data[off + 2], data[off + 3],
                    data[off + 4], data[off + 5], data[off + 6], data[off + 7],
                ])
            })
            .collect()
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

fn compute_metrics(closes: &[f64], signals: &[f64], charges: &BrokerCharges) -> Metrics {
    let mut num_trades = 0i32;
    let mut wins = 0i32;
    let mut total_pnl = 0.0f64;
    let mut entry_price = 0.0f64;
    let mut position: f64 = 0.0;
    let mut cum_pnl = 0.0f64;
    let mut peak = 0.0f64;
    let mut max_drawdown = 0.0f64;

    let mut record = |gross_pnl: f64, buy_price: f64, sell_price: f64, size: f64| {
        let cost = charges.cost(buy_price, sell_price) * size;
        let pnl = gross_pnl - cost;
        total_pnl += pnl;
        num_trades += 1;
        if pnl > 0.0 { wins += 1; }
        cum_pnl += pnl;
        if cum_pnl > peak { peak = cum_pnl; }
        let dd = peak - cum_pnl;
        if dd > max_drawdown { max_drawdown = dd; }
    };

    let mut prev_target: f64 = 0.0;
    for (i, &target) in signals.iter().enumerate() {
        let target = target.clamp(-1.0, 1.0);
        if target == prev_target { prev_target = target; continue; }

        if target != position {
            if position > 0.0 && target < position {
                let closed = position - target.max(0.0);
                record(closed * (closes[i] - entry_price), entry_price, closes[i], closed);
            } else if position < 0.0 && target > position {
                let closed = position.abs() - target.min(0.0).abs();
                record(closed * (entry_price - closes[i]), closes[i], entry_price, closed);
            }
            if target == 0.0 {
                entry_price = 0.0;
            } else if position == 0.0 {
                entry_price = closes[i];
            } else if position.signum() != target.signum() {
                entry_price = closes[i];
            } else {
                let same_dir = if position > 0.0 { position.min(target) } else { position.max(target) };
                let added = (target - position).abs();
                let new_size = target.abs();
                if new_size > 0.0 {
                    entry_price = (entry_price * same_dir.abs() + closes[i] * added) / new_size;
                }
            }
        }
        position = target;
        prev_target = target;
    }

    if position != 0.0 {
        if let Some(&last) = closes.last() {
            if position > 0.0 {
                record(position * (last - entry_price), entry_price, last, position);
            } else {
                record(position.abs() * (entry_price - last), last, entry_price, position.abs());
            }
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
    signals: &[f64],
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
        Field::new("signal", DataType::Float64, false),
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
        Arc::new(Float64Array::from(signals.to_vec())),
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
        "select r.interval, r.from_date::text, r.to_date::text, s.wasm_key, s.rl_config \
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

    let rl_config: serde_json::Value = run_row.get(4);
    let lookback = rl_config["lookback_candles"].as_u64().unwrap_or(20) as usize;
    let mut max_period = lookback;
    if let Some(arr) = rl_config["indicators"].as_array() {
        for ind in arr {
            if let Some(p) = ind["period"].as_u64() {
                max_period = max_period.max(p as usize);
            }
            if let Some(p) = ind["fast"].as_u64() {
                max_period = max_period.max(p as usize);
            }
            if let Some(p) = ind["slow"].as_u64() {
                max_period = max_period.max(p as usize);
            }
            if let Some(p) = ind["signal_period"].as_u64() {
                max_period = max_period.max(p as usize);
            }
        }
    }
    let warmup_needed = lookback + max_period;
    let buffer_days = (warmup_needed as f64 * 1.5).ceil() as i64;
    let fetch_from = chrono::NaiveDate::parse_from_str(&from_date, "%Y-%m-%d")
        .map_err(|e| e.to_string())?
        .checked_sub_signed(chrono::Duration::days(buffer_days))
        .ok_or("invalid from_date")?;
    let fetch_from_str = fetch_from.format("%Y-%m-%d").to_string();

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
        extra_count: usize,
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
            &[&security_id.as_str(), &exchange_segment.as_str(), &interval, &fetch_from_str, &to_date],
        ).await.map_err(|e| e.to_string())?;

        if candle_rows.is_empty() { continue; }

        let mut ic = InstCandles {
            security_id, exchange_segment,
            timestamps: vec![], opens: vec![], highs: vec![],
            lows: vec![], closes: vec![], volumes: vec![],
            extra_count: 0,
        };
        for r in &candle_rows {
            ic.timestamps.push(r.get(0));
            ic.opens.push(r.get(1));
            ic.highs.push(r.get(2));
            ic.lows.push(r.get(3));
            ic.closes.push(r.get(4));
            ic.volumes.push(r.get(5));
        }
        let from_naive = chrono::NaiveDate::parse_from_str(&from_date, "%Y-%m-%d").unwrap();
        ic.extra_count = ic.timestamps.iter()
            .take_while(|&&ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.date_naive() < from_naive)
                    .unwrap_or(false)
            })
            .count();
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
        signals: Vec<f64>,
        metrics: Metrics,
    }

    let results: Result<Vec<InstResult>, String> = inst_candles
        .into_par_iter()
        .map(|ic| {
            let volumes_f64: Vec<f64> = ic.volumes.iter().map(|&v| v as f64).collect();
            let all_signals = run_wasm(&wasm, &ic.closes, &ic.opens, &volumes_f64, &ic.highs, &ic.lows)?;
            let extra = ic.extra_count;
            let signals = all_signals[extra..].to_vec();
            let metrics = compute_metrics(&ic.closes[extra..], &signals, &charges.clone());
            Ok(InstResult {
                security_id: ic.security_id,
                exchange_segment: ic.exchange_segment,
                timestamps: ic.timestamps[extra..].to_vec(),
                opens: ic.opens[extra..].to_vec(),
                highs: ic.highs[extra..].to_vec(),
                lows: ic.lows[extra..].to_vec(),
                closes: ic.closes[extra..].to_vec(),
                volumes: ic.volumes[extra..].to_vec(),
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
    let mut col_sig: Vec<f64> = vec![];
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
            .filter(|(_, &sig)| sig != 0.0)
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
indicators = {{ path = "/rust/indicators" }}
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

async fn compile_snippet(
    s3: &S3Client,
    bucket: &str,
    strategy_id: &str,
    snippet: &str,
) -> Result<String, String> {
    let tmp_id = uuid::Uuid::new_v4().to_string();
    let dir = format!("/tmp/strategy_{}", tmp_id);
    fs::create_dir_all(format!("{}/src", dir)).map_err(|e| e.to_string())?;

    fs::write(format!("{}/Cargo.toml", dir), format!(r#"
[package]
name = "strategy"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
indicators = {{ path = "/rust/indicators" }}
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

    let wasm_path = format!("{}/target/wasm32-unknown-unknown/release/strategy.wasm", dir);
    let wasm = fs::read(&wasm_path).map_err(|e| e.to_string())?;
    let wasm_key = format!("strategies/{}/strategy.wasm", strategy_id);
    upload(s3, bucket, &wasm_key, wasm).await?;

    fs::remove_dir_all(&dir).ok();
    Ok(wasm_key)
}

async fn process_rl_job(
    db: &tokio_postgres::Client,
    s3: &S3Client,
    bucket: &str,
    job_id: uuid::Uuid,
    strategy_id: uuid::Uuid,
) -> Result<(), String> {
    db.execute(
        "update rl_jobs set status='training', updated_at=now() where id=$1",
        &[&job_id],
    ).await.map_err(|e| e.to_string())?;

    let row = db.query_one(
        "select rl_config from strategies where id=$1",
        &[&strategy_id],
    ).await.map_err(|e| e.to_string())?;

    let rl_config: serde_json::Value = row.get(0);

    let train_from: String = rl_config["train_from"].as_str().unwrap_or("").to_string();
    let train_to: String = rl_config["train_to"].as_str().unwrap_or("").to_string();
    let external_test_from: Option<String> = rl_config["test_from"].as_str().map(|s| s.to_string());
    let external_test_to: Option<String> = rl_config["test_to"].as_str().map(|s| s.to_string());
    let lookback = rl_config["lookback_candles"].as_u64().unwrap_or(20) as usize;
    let allow_short = rl_config["allow_short"].as_bool().unwrap_or(false);
    let reward_type = rl_config["reward"].as_str().unwrap_or("pnl").to_string();
    let training_method = rl_config["training_method"].as_str().unwrap_or("ppo").to_string();
    let lr = rl_config["lr"].as_f64().unwrap_or(1e-4);
    let ppo_epochs = rl_config["ppo_epochs"].as_u64().unwrap_or(4) as usize;
    let clip_epsilon = rl_config["clip_epsilon"].as_f64().unwrap_or(0.2);
    let value_coef = rl_config["value_coef"].as_f64().unwrap_or(0.5);
    let entropy_coef = rl_config["entropy_coef"].as_f64().unwrap_or(0.01);
    let gae_lambda = rl_config["gae_lambda"].as_f64().unwrap_or(0.95);
    let batch_episodes = rl_config["batch_episodes"].as_u64().unwrap_or(8) as usize;
    let hidden_size = rl_config["hidden_size"].as_u64().unwrap_or(64) as usize;
    let num_layers = rl_config["num_layers"].as_u64().unwrap_or(2) as usize;
    let activation = rl_config["activation"].as_str().unwrap_or("relu").to_string();
    let reward_norm = rl_config["reward_norm"].as_bool().unwrap_or(true);
    let lr_schedule = rl_config["lr_schedule"].as_bool().unwrap_or(true);
    let entropy_anneal = rl_config["entropy_anneal"].as_bool().unwrap_or(true);
    let regularization_type = rl_config["regularization_type"].as_str().unwrap_or("none").to_string();
    let regularization_lambda = rl_config["regularization_lambda"].as_f64().unwrap_or(0.0);
    let continuous_action = rl_config["continuous_action"].as_bool().unwrap_or(false);
    let action_std = rl_config["action_std"].as_f64().unwrap_or(0.3);

    let indicator_specs: Vec<IndicatorSpec> = serde_json::from_value(
        rl_config["indicators"].clone()
    ).map_err(|e| e.to_string())?;

    let mut penalty_holding = None;
    let mut max_holding_days = None;
    let mut penalty_trades = None;
    let mut max_trades_per_month = None;
    if let Some(arr) = rl_config["constraints"].as_array() {
        for c in arr {
            match c["type"].as_str().unwrap_or("") {
                "max_holding_days" => {
                    max_holding_days = c["value"].as_u64().map(|v| v as usize);
                    penalty_holding = Some(0.01);
                }
                "max_trades_per_month" => {
                    max_trades_per_month = c["value"].as_u64().map(|v| v as usize);
                    penalty_trades = Some(0.01);
                }
                _ => {}
            }
        }
    }

    let security_id = rl_config["security_id"].as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "No instrument configured in RL config".to_string())?
        .to_string();
    let exchange_segment = rl_config["exchange_segment"].as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "No exchange segment configured in RL config".to_string())?
        .to_string();

    // Broker charges (daily = delivery)
    let charge_row = db.query_one(
        "select brokerage_flat::float8, brokerage_pct::float8, stt_buy_pct::float8, \
                stt_sell_pct::float8, exchange_pct::float8, sebi_pct::float8, \
                stamp_buy_pct::float8, gst_pct::float8 \
         from broker_charges where trade_type = 'delivery'",
        &[],
    ).await.map_err(|e| e.to_string())?;
    let charges = rl::train::BrokerCharges {
        brokerage_flat: charge_row.get(0),
        brokerage_pct:  charge_row.get(1),
        stt_buy_pct:    charge_row.get(2),
        stt_sell_pct:   charge_row.get(3),
        exchange_pct:   charge_row.get(4),
        sebi_pct:       charge_row.get(5),
        stamp_buy_pct:  charge_row.get(6),
        gst_pct:        charge_row.get(7),
    };

    let fetch_candles = |from: &str, to: &str| {
        let from = from.to_string();
        let to = to.to_string();
        let sid = security_id.clone();
        let seg = exchange_segment.clone();
        async move {
            db.query(
                "select extract(epoch from timestamp)::bigint, open::float8, high::float8,
                        low::float8, close::float8, volume
                 from candles
                 where security_id=$1 and exchange_segment=$2 and interval='day'
                 and timestamp::date between $3::text::date and $4::text::date
                 order by timestamp",
                &[&sid, &seg, &from, &to],
            ).await.map_err(|e: tokio_postgres::Error| e.to_string())
        }
    };

    let to_candles = |rows: Vec<tokio_postgres::Row>| Candles {
        opens:   rows.iter().map(|r| r.get::<_, f64>(1)).collect(),
        highs:   rows.iter().map(|r| r.get::<_, f64>(2)).collect(),
        lows:    rows.iter().map(|r| r.get::<_, f64>(3)).collect(),
        closes:  rows.iter().map(|r| r.get::<_, f64>(4)).collect(),
        volumes: rows.iter().map(|r| r.get::<_, i64>(5) as f64).collect(),
    };
    let epoch_to_date = |ts: i64| {
        DateTime::<Utc>::from_timestamp(ts, 0)
            .map(|dt| dt.date_naive().to_string())
            .unwrap_or_else(|| "".to_string())
    };

    let train_rows = fetch_candles(&train_from, &train_to).await?;
    if train_rows.is_empty() {
        return Err(format!("No daily candles found for {} {} in {} to {}", security_id, exchange_segment, train_from, train_to));
    }
    let train_epochs: Vec<i64> = train_rows.iter().map(|r| r.get::<_, i64>(0)).collect();
    let train_candles = to_candles(train_rows);

    let indicator_series = compute_indicators(&train_candles, &indicator_specs);
    let (raw_states, mut feature_names, candle_indices) = build_state_matrix_with_indices(&train_candles, &indicator_series, lookback);
    if raw_states.nrows() == 0 {
        return Err(format!("Not enough usable candles after indicator warmup for {} {} in {} to {}", security_id, exchange_segment, train_from, train_to));
    }
    let (train_end, val_end) = split_points(raw_states.nrows());
    let mut train_fit_states = raw_states.slice(ndarray::s![..train_end, ..]).to_owned();
    let (means, stds) = normalise_with_stats(&mut train_fit_states);
    let mut states = raw_states;
    apply_normalisation_stats(&mut states, &means, &stds)?;
    feature_names.extend([
        "position".to_string(),
        "holding_days".to_string(),
        "unrealized_pnl".to_string(),
    ]);

    let cfg = TrainConfig {
        max_episodes: 2000,
        episode_steps: states.nrows().min(200),
        validation_interval: 10,
        early_stopping_patience: 100,
        min_delta: 1e-6,
        grad_clip_norm: 1.0,
        lr,
        gamma: 0.99,
        allow_short,
        reward_type,
        penalty_holding_days: penalty_holding,
        max_holding_days,
        penalty_trades_per_month: penalty_trades,
        max_trades_per_month,
        training_method: training_method.clone(),
        ppo_epochs,
        clip_epsilon,
        value_coef,
        entropy_coef,
        gae_lambda,
        batch_episodes,
        hidden_size,
        num_layers,
        activation,
        reward_norm,
        lr_schedule,
        entropy_anneal,
        regularization_type,
        regularization_lambda,
        continuous_action,
        action_std,
    };

    let result = if training_method == "reinforce" {
        train_reinforce(&states, &train_candles.closes, &candle_indices, &cfg, &charges)
    } else {
        train_ppo(&states, &train_candles.closes, &candle_indices, &cfg, &charges)
    };
    let external_test_pnl: Option<f64> = if let (Some(ref tf), Some(ref tt)) = (&external_test_from, &external_test_to) {
        let test_rows = fetch_candles(tf, tt).await?;
        if test_rows.is_empty() {
            None
        } else {
            let test_candles = to_candles(test_rows);
            let test_ind = compute_indicators(&test_candles, &indicator_specs);
            let (mut test_states, _, test_indices) = build_state_matrix_with_indices(&test_candles, &test_ind, lookback);
            apply_normalisation_stats(&mut test_states, &means, &stds)?;
            if test_states.nrows() == 0 {
                None
            } else {
                Some(rl::train::evaluate(&result.net, &test_states, &test_candles.closes, &test_indices, allow_short, &charges))
            }
        }
    } else {
        None
    };

    let weights = weights_to_bytes(&result.net)?;
    let weights_key = format!("strategies/{}/weights.bin", strategy_id);
    upload(s3, bucket, &weights_key, weights).await?;

    let train_states_for_explain = collect_greedy_states(
        &result.net,
        &states.slice(ndarray::s![..train_end, ..]).to_owned(),
        &train_candles.closes,
        &candle_indices[..train_end],
        allow_short,
    );
    let raw_imp = feature_importance(&result.net, &train_states_for_explain);
    let norm_imp = normalise_importance(&raw_imp);
    let feature_importance_json: Vec<serde_json::Value> = feature_names.iter()
        .zip(norm_imp.iter())
        .map(|(name, &imp)| serde_json::json!({ "name": name, "importance": imp }))
        .collect();

    let approx_rules = distil(&result.net, &train_states_for_explain, &feature_names, 3);

    let rust_snippet = net_to_rust(
        &result.net, &indicator_specs, lookback, &means, &stds, allow_short,
    );
    let split_date = |state_row: usize| -> String {
        candle_indices.get(state_row)
            .and_then(|&idx| train_epochs.get(idx))
            .map(|&ts| epoch_to_date(ts))
            .unwrap_or_default()
    };

    let rl_summary = serde_json::json!({
        "feature_importance": feature_importance_json,
        "approximate_rules": approx_rules,
        "training_episodes": result.episodes,
        "best_episode": result.best_episode,
        "final_train_reward": result.final_train_reward,
        "train_pnl": result.train_pnl,
        "val_pnl": result.val_pnl,
        "test_pnl": external_test_pnl.unwrap_or(result.test_pnl),
        "internal_test_pnl": result.test_pnl,
        "external_test_pnl": external_test_pnl,
        "split": {
            "train_from": split_date(0),
            "train_to": split_date(train_end.saturating_sub(1)),
            "val_from": split_date(train_end),
            "val_to": split_date(val_end.saturating_sub(1)),
            "test_from": split_date(val_end),
            "test_to": split_date(states.nrows().saturating_sub(1)),
            "train_rows": train_end,
            "val_rows": val_end.saturating_sub(train_end),
            "test_rows": states.nrows().saturating_sub(val_end),
        },
    });

    db.execute(
        "update strategies set rl_summary=$1 where id=$2",
        &[&rl_summary, &strategy_id],
    ).await.map_err(|e| e.to_string())?;

    // Store per-episode training metrics
    for m in &result.metrics {
        db.execute(
            "insert into rl_training_metrics (strategy_id, episode, train_reward, val_metric) values ($1, $2, $3, $4)",
            &[&strategy_id, &(m.episode as i32), &m.train_reward, &m.val_metric],
        ).await.map_err(|e| format!("failed to insert training metric: {}", e))?;
    }

    match compile_snippet(s3, bucket, &strategy_id.to_string(), &rust_snippet).await {
        Ok(wasm_key) => {
            db.execute(
                "update strategies set wasm_key=$1 where id=$2",
                &[&wasm_key, &strategy_id],
            ).await.map_err(|e| e.to_string())?;
        }
        Err(e) => {
            eprintln!("RL distil compile failed (strategy still trained): {}", e);
        }
    }

    db.execute(
        "update rl_jobs set status='done', updated_at=now() where id=$1",
        &[&job_id],
    ).await.map_err(|e| e.to_string())?;

    Ok(())
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

        // rl jobs
        let rl_rows = db.query(
            "select id, strategy_id from rl_jobs where status = 'pending' order by created_at limit 1",
            &[],
        ).await.unwrap_or_default();

        for row in rl_rows {
            let job_id: uuid::Uuid = row.get(0);
            let strategy_id: uuid::Uuid = row.get(1);

            println!("builder: starting rl training for strategy {}", strategy_id);

            match process_rl_job(&db, &s3, &bucket, job_id, strategy_id).await {
                Ok(()) => println!("builder: rl training done {}", strategy_id),
                Err(e) => {
                    eprintln!("builder: rl training failed {}: {}", strategy_id, e);
                    db.execute(
                        "update rl_jobs set status='failed', error=$1, updated_at=now() where id=$2",
                        &[&e, &job_id],
                    ).await.ok();
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}
