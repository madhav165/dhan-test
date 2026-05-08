use std::env;
use std::time::Duration;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use serde::Deserialize;
use tokio_postgres::NoTls;

const POLL_INTERVAL_SECS: u64 = 3;
const ORDER_POLL_ATTEMPTS: u32 = 20;
const ORDER_POLL_DELAY_SECS: u64 = 3;
// Terminal order statuses from Dhan
const TERMINAL: &[&str] = &["TRADED", "PART_TRADED", "REJECTED", "CANCELLED", "EXPIRED"];

struct Config {
    db_url: String,
    dhan_base_url: String,
    telegram_bot_token: String,
    enc_key: Vec<u8>,
}

// ── db helpers ───────────────────────────────────────────────────────────────

fn decrypt_token(key: &[u8], encrypted_hex: &str) -> Result<String, String> {
    let data = hex::decode(encrypted_hex).map_err(|e| e.to_string())?;
    if data.len() < 12 { return Err("ciphertext too short".into()); }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plain = cipher.decrypt(nonce, ciphertext).map_err(|e| e.to_string())?;
    String::from_utf8(plain).map_err(|e| e.to_string())
}

async fn get_broker_token(
    db: &tokio_postgres::Client,
    enc_key: &[u8],
    user_id: &str,
) -> Result<(String, String), String> {
    let row = db.query_opt(
        "select client_id, encrypted_token::text from broker_connections
         where user_id = $1 and is_active = true
         and token_date = (current_timestamp at time zone 'Asia/Kolkata')::date",
        &[&user_id],
    ).await.map_err(|e| e.to_string())?
        .ok_or_else(|| "no active token".to_string())?;
    let client_id: String = row.get(0);
    let enc_token: String = row.get(1);
    let access_token = decrypt_token(enc_key, &enc_token)?;
    Ok((client_id, access_token))
}

// ── dhan API calls ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct FundLimit {
    #[serde(rename = "availabelBalance")]
    available_balance: f64,
}

async fn check_funds(
    dhan_base_url: &str,
    client_id: &str,
    access_token: &str,
) -> Result<f64, String> {
    let resp = reqwest::Client::new()
        .get(format!("{dhan_base_url}/fundlimit"))
        .header("access-token", access_token)
        .header("client-id", client_id)
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("fundlimit status {}", resp.status()));
    }
    let f: FundLimit = resp.json().await.map_err(|e| e.to_string())?;
    Ok(f.available_balance)
}

#[derive(Deserialize)]
struct MarginResp {
    #[serde(rename = "totalMargin")]
    total_margin: f64,
    #[serde(rename = "availableBalance")]
    available_balance: f64,
}

fn map_segment(seg: &str) -> &'static str {
    match seg {
        "NSE_E" => "NSE_EQ",
        "BSE_E" => "BSE_EQ",
        "NSE_I" => "IDX_I",
        _ => "NSE_EQ",
    }
}

async fn check_margin(
    dhan_base_url: &str,
    client_id: &str,
    access_token: &str,
    sec_id: &str,
    seg: &str,
    transaction_type: &str,
    quantity: i32,
    price: f64,
    order_type: &str,
    product_type: &str,
) -> Result<MarginResp, String> {
    let payload = serde_json::json!({
        "dhanClientId": client_id,
        "exchangeSegment": map_segment(seg),
        "transactionType": transaction_type,
        "quantity": quantity,
        "productType": product_type,
        "securityId": sec_id,
        "price": price,
        "orderType": order_type,
    });
    let resp = reqwest::Client::new()
        .post(format!("{dhan_base_url}/margincalculator"))
        .header("access-token", access_token)
        .header("client-id", client_id)
        .json(&payload)
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("margincalculator status {}", resp.status()));
    }
    resp.json::<MarginResp>().await.map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct PlaceOrderResp {
    #[serde(rename = "orderId")]
    order_id: String,
}

async fn place_order(
    dhan_base_url: &str,
    client_id: &str,
    access_token: &str,
    sec_id: &str,
    seg: &str,
    transaction_type: &str,
    quantity: i32,
    price: f64,
    order_type: &str,
    product_type: &str,
    correlation_id: &str,
) -> Result<String, String> {
    let payload = serde_json::json!({
        "dhanClientId": client_id,
        "correlationId": &correlation_id[..correlation_id.len().min(30)],
        "transactionType": transaction_type,
        "exchangeSegment": map_segment(seg),
        "productType": product_type,
        "orderType": order_type,
        "validity": "DAY",
        "securityId": sec_id,
        "quantity": quantity,
        "price": if order_type == "MARKET" { 0.0 } else { price },
        "triggerPrice": 0,
        "afterMarketOrder": false,
    });
    let resp = reqwest::Client::new()
        .post(format!("{dhan_base_url}/orders"))
        .header("access-token", access_token)
        .header("client-id", client_id)
        .json(&payload)
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("place order failed: {body}"));
    }
    let r: PlaceOrderResp = resp.json().await.map_err(|e| e.to_string())?;
    Ok(r.order_id)
}

#[derive(Deserialize)]
struct OrderStatusResp {
    #[serde(rename = "orderStatus")]
    order_status: String,
    #[serde(rename = "tradedPrice", default)]
    traded_price: f64,
    #[serde(rename = "quantity", default)]
    quantity: i32,
}

async fn poll_order_status(
    dhan_base_url: &str,
    client_id: &str,
    access_token: &str,
    order_id: &str,
) -> Result<OrderStatusResp, String> {
    let resp = reqwest::Client::new()
        .get(format!("{dhan_base_url}/orders/{order_id}"))
        .header("access-token", access_token)
        .header("client-id", client_id)
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("order status failed: {}", resp.status()));
    }
    resp.json::<OrderStatusResp>().await.map_err(|e| e.to_string())
}

// ── telegram ─────────────────────────────────────────────────────────────────

async fn send_telegram(bot_token: &str, chat_id: &str, text: &str) {
    if bot_token.is_empty() || chat_id.is_empty() { return; }
    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
    let _ = reqwest::Client::new()
        .post(&url)
        .json(&serde_json::json!({"chat_id": chat_id, "text": text}))
        .send().await;
}

// ── job processing ───────────────────────────────────────────────────────────

async fn process_job(db: &tokio_postgres::Client, cfg: &Config, job_id: &str) {
    // Load job + policy + user details in one query
    let row = match db.query_opt(
        "select j.policy_id::text, j.security_id, j.exchange_segment, j.signal,
                j.price::float8, j.quantity, j.order_type, j.product_type, j.correlation_id,
                u.id::text, u.telegram_chat_id,
                p.max_trade_value::float8
         from trade_jobs j
         join policies p on p.id = j.policy_id
         join strategies s on s.id = p.strategy_id
         join users u on u.id = s.user_id
         where j.id = $1",
        &[&job_id],
    ).await {
        Ok(Some(r)) => r,
        _ => { fail(db, job_id, "job lookup failed", None).await; return; }
    };

    let policy_id: String = row.get(0);
    let security_id: String = row.get(1);
    let exchange_segment: String = row.get(2);
    let transaction_type: String = row.get(3); // "BUY" | "SELL"
    let price: f64 = row.get(4);
    let quantity: i32 = row.get(5);
    let order_type: String = row.get(6);
    let product_type: String = row.get(7);
    let correlation_id: String = row.get(8);
    let user_id: String = row.get(9);
    let telegram_chat_id: Option<String> = row.get(10);
    let max_trade_value: f64 = row.get(11);

    set_status(db, job_id, "checking").await;

    // 1. max_trade_value check (0 = no limit)
    let trade_value = price * quantity as f64;
    if max_trade_value > 0.0 && trade_value > max_trade_value {
        let msg = format!(
            "❌ Trade rejected: {} {} value {:.0} exceeds limit {:.0}",
            transaction_type, security_id, trade_value, max_trade_value
        );
        if let Some(ref chat_id) = telegram_chat_id {
            send_telegram(&cfg.telegram_bot_token, chat_id, &msg).await;
        }
        fail(db, job_id, &msg, telegram_chat_id.as_deref()).await;
        return;
    }

    // 2. Broker token
    let (client_id, access_token) = match get_broker_token(db, &cfg.enc_key, &user_id).await {
        Ok(t) => t,
        Err(e) => { fail(db, job_id, &format!("token error: {e}"), telegram_chat_id.as_deref()).await; return; }
    };

    // 3. Margin check
    match check_margin(
        &cfg.dhan_base_url, &client_id, &access_token,
        &security_id, &exchange_segment, &transaction_type,
        quantity, price, &order_type, &product_type,
    ).await {
        Ok(m) if m.available_balance < m.total_margin => {
            let msg = format!(
                "❌ Trade rejected: {} {} insufficient funds (need {:.0}, have {:.0})",
                transaction_type, security_id, m.total_margin, m.available_balance
            );
            if let Some(ref chat_id) = telegram_chat_id {
                send_telegram(&cfg.telegram_bot_token, chat_id, &msg).await;
            }
            fail(db, job_id, &msg, telegram_chat_id.as_deref()).await;
            return;
        }
        Err(e) => {
            fail(db, job_id, &format!("margin check error: {e}"), telegram_chat_id.as_deref()).await;
            return;
        }
        Ok(_) => {}
    }

    // 4. Check for existing open position — close it if signal reverses
    let existing = db.query_opt(
        "select direction, quantity from trade_positions
         where policy_id = $1 and security_id = $2 and exchange_segment = $3",
        &[&policy_id, &security_id, &exchange_segment],
    ).await.ok().flatten();

    if let Some(pos) = &existing {
        let pos_direction: String = pos.get(0);
        let pos_qty: i32 = pos.get(1);
        // Signal direction matches open position — already in this trade, skip
        let signal_direction = if transaction_type == "BUY" { "LONG" } else { "SHORT" };
        if pos_direction == signal_direction {
            set_status(db, job_id, "done").await;
            return;
        }
        // Reverse signal — close the position first with opposite order
        let close_type = if pos_direction == "LONG" { "SELL" } else { "BUY" };
        set_status(db, job_id, "placing").await;
        let close_correlation = format!("close-{}", &correlation_id[..correlation_id.len().min(24)]);
        match place_order(
            &cfg.dhan_base_url, &client_id, &access_token,
            &security_id, &exchange_segment,
            close_type, pos_qty, price, &order_type, &product_type,
            &close_correlation,
        ).await {
            Ok(order_id) => {
                wait_for_fill(db, &cfg.dhan_base_url, &client_id, &access_token, job_id, &order_id).await;
                // Remove the closed position
                let _ = db.execute(
                    "delete from trade_positions where policy_id=$1 and security_id=$2 and exchange_segment=$3",
                    &[&policy_id, &security_id, &exchange_segment],
                ).await;
            }
            Err(e) => {
                fail(db, job_id, &format!("close order failed: {e}"), telegram_chat_id.as_deref()).await;
                return;
            }
        }
    }

    // 5. Place the new order
    set_status(db, job_id, "placing").await;
    let order_id = match place_order(
        &cfg.dhan_base_url, &client_id, &access_token,
        &security_id, &exchange_segment,
        &transaction_type, quantity, price, &order_type, &product_type,
        &correlation_id,
    ).await {
        Ok(id) => id,
        Err(e) => {
            fail(db, job_id, &format!("place order error: {e}"), telegram_chat_id.as_deref()).await;
            return;
        }
    };

    db.execute(
        "update trade_jobs set order_id=$1, status='polling', updated_at=now() where id=$2",
        &[&order_id, &job_id],
    ).await.ok();

    // 6. Poll for terminal status
    let final_status = wait_for_fill(db, &cfg.dhan_base_url, &client_id, &access_token, job_id, &order_id).await;

    // 7. Record position or alert on failure
    let direction = if transaction_type == "BUY" { "LONG" } else { "SHORT" };
    if final_status == "TRADED" || final_status == "PART_TRADED" {
        let _ = db.execute(
            "insert into trade_positions (policy_id, security_id, exchange_segment, direction, quantity, entry_price)
             values ($1, $2, $3, $4, $5, $6)
             on conflict (policy_id, security_id, exchange_segment)
             do update set direction=excluded.direction, quantity=excluded.quantity,
                           entry_price=excluded.entry_price, opened_at=now()",
            &[&policy_id, &security_id, &exchange_segment, &direction, &quantity, &price],
        ).await;
        if let Some(ref chat_id) = telegram_chat_id {
            let msg = format!(
                "✅ Trade filled: {} {} qty={} @ {:.2}",
                direction, security_id, quantity, price
            );
            send_telegram(&cfg.telegram_bot_token, chat_id, &msg).await;
        }
    } else {
        let msg = format!(
            "❌ Trade {} for {} {}: order status {}",
            transaction_type, security_id, exchange_segment, final_status
        );
        if let Some(ref chat_id) = telegram_chat_id {
            send_telegram(&cfg.telegram_bot_token, chat_id, &msg).await;
        }
        fail(db, job_id, &msg, None).await;
    }
}

/// Poll order status until terminal or max attempts. Returns final status string.
async fn wait_for_fill(
    db: &tokio_postgres::Client,
    dhan_base_url: &str,
    client_id: &str,
    access_token: &str,
    job_id: &str,
    order_id: &str,
) -> String {
    for _ in 0..ORDER_POLL_ATTEMPTS {
        tokio::time::sleep(Duration::from_secs(ORDER_POLL_DELAY_SECS)).await;
        match poll_order_status(dhan_base_url, client_id, access_token, order_id).await {
            Ok(s) => {
                db.execute(
                    "update trade_jobs set order_status=$1, updated_at=now() where id=$2",
                    &[&s.order_status, &job_id],
                ).await.ok();
                if TERMINAL.contains(&s.order_status.as_str()) {
                    return s.order_status;
                }
            }
            Err(e) => eprintln!("poll order {order_id}: {e}"),
        }
    }
    "TIMEOUT".to_string()
}

async fn set_status(db: &tokio_postgres::Client, job_id: &str, status: &str) {
    db.execute(
        "update trade_jobs set status=$1, updated_at=now() where id=$2",
        &[&status, &job_id],
    ).await.ok();
}

async fn fail(db: &tokio_postgres::Client, job_id: &str, error: &str, _chat_id: Option<&str>) {
    db.execute(
        "update trade_jobs set status='failed', error=$1, updated_at=now() where id=$2",
        &[&error, &job_id],
    ).await.ok();
}

// ── main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cfg = Config {
        db_url: env::var("DATABASE_URL").expect("DATABASE_URL required"),
        dhan_base_url: env::var("DHAN_BASE_URL").expect("DHAN_BASE_URL required"),
        telegram_bot_token: env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default(),
        enc_key: hex::decode(env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY required"))
            .expect("ENCRYPTION_KEY must be hex"),
    };

    let (db, connection) = tokio_postgres::connect(&cfg.db_url, NoTls).await
        .expect("db connect failed");
    tokio::spawn(async move { connection.await.expect("db connection error") });

    println!("trade-worker: polling for jobs");

    loop {
        let rows = db.query(
            "select id::text from trade_jobs where status = 'pending'
             order by created_at limit 1",
            &[],
        ).await.unwrap_or_default();

        for row in rows {
            let job_id: String = row.get(0);
            println!("trade-worker: processing job {job_id}");
            process_job(&db, &cfg, &job_id).await;
        }

        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
}
