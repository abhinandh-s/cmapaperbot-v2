use serde::{Deserialize, Serialize};
use tracing_subscriber::{filter::LevelFilter, fmt::time::UtcTime, prelude::*};
use tracing_web::MakeConsoleWriter;
use worker::*;

mod consts;

// ---------- Telegram types (only the fields we need) ----------
// Serde ignores unknown JSON fields, so Telegram adding fields won't break us.

#[derive(Deserialize)]
struct Update {
    update_id: i64,
    message: Option<Message>, // a given update carries only one kind of payload
}
#[derive(Deserialize)]
struct Message {
    chat: Chat,
    text: Option<String>, // photos, documents etc. have no text
}
#[derive(Deserialize)]
struct Chat {
    id: i64,
}

// Returned as the webhook response body. The `method` field tells Telegram
// "perform this API call for me", so we need no outbound HTTP client yet.
#[derive(Serialize)]
struct SendMessage {
    method: &'static str,
    chat_id: i64,
    text: String,
}

macro_rules! send_message {
    ($chat_id:expr, $text:expr) => {
        SendMessage {
            method: "sendMessage",
            chat_id: $chat_id,
            text: $text.into(),
        }
    };
    ($method:expr, $chat_id:expr, $text:expr) => {
        SendMessage {
            method: $method,
            chat_id: $chat_id,
            text: $text.into(),
        }
    };
}

// ---------- Tracing setup: runs once when the Worker loads ----------

#[event(start)]
fn start() {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_ansi(false)
        .with_timer(UtcTime::rfc_3339())
        .with_writer(MakeConsoleWriter); // -> console.*, which Workers captures
    tracing_subscriber::registry()
        .with(LevelFilter::INFO)
        .with(fmt_layer)
        .init(); // panics if called twice, hence `start`
}

// ---------- HTTP entry point ----------

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .post_async("/webhook", webhook)
        .run(req, env)
        .await
}

async fn webhook(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // 1. Only accept requests carrying the secret we gave Telegram in setWebhook.
    let expected = ctx.var("TELEGRAM_TOKEN")?.to_string();
    let got = req.headers().get("X-Telegram-Bot-Api-Secret-Token")?;
    if got.as_deref() != Some(expected.as_str()) {
        tracing::warn!("rejected request: bad or missing secret token");
        return Response::error("forbidden", 403);
    }

    // 2. Parse the update. On failure still return 200, or Telegram retries it.
    let update: Update = match req.json().await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = %e, "could not parse update");
            return Response::ok("");
        }
    };

    // 3. Decide on a reply.
    match handle(&update) {
        Some(reply) => Response::from_json(&reply), // sets Content-Type: application/json
        None => Response::ok(""),
    }
}

// One span per update; every log line inside carries update_id.
// skip_all = don't auto-record arguments (they could contain user data).
#[tracing::instrument(skip_all, fields(update_id = update.update_id))]
fn handle(update: &Update) -> Option<SendMessage> {
    let msg = update.message.as_ref()?;
    let text = msg.text.as_deref()?;

    // "/start", "/start payload", and "/start@mybot" all count as /start
    let command = text.split_whitespace().next()?.split('@').next()?;
    tracing::info!(chat_id = msg.chat.id, command, "received message");

    match command {
        "/start" => Some(send_message!(msg.chat.id, consts::start())),
       "/start_old" => Some(SendMessage {
            method: "sendMessage",
            chat_id: msg.chat.id,
            text: "Hello! I'm alive on Cloudflare Workers.".to_string(),
        }),
        _ => {
            tracing::info!("unhandled command, ignoring");
            None
        }
    }
}