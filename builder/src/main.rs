use std::env;
use std::fs;
use std::process::Command;
use std::time::Duration;
use tokio_postgres::NoTls;
use aws_sdk_s3::{Client as S3Client, config::Builder as S3Builder};
use aws_credential_types::Credentials;
use aws_config::Region;

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

    let (db, connection) = tokio_postgres::connect(&db_url, NoTls).await.expect("db connect failed");
    tokio::spawn(async move { connection.await.expect("db connection error") });

    let s3 = s3_client().await;

    // Ensure bucket exists
    s3.create_bucket().bucket(&bucket).send().await.ok();

    println!("builder: polling for jobs");

    loop {
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

        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}
