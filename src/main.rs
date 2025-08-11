use std::{env, sync::LazyLock};

use axum::{
    body::{to_bytes, Body},
    routing::post,
    Router,
};
use flate2::bufread::GzDecoder;
use jiff::SignedDuration;
use pushover_rs::{send_pushover_request, MessageBuilder};
use serde::Deserialize;
use serde_json::de::Deserializer;
use tokio::net::TcpListener;

static PUSHOVER_USER: LazyLock<String> = LazyLock::new(|| env::var("PUSHOVER_USER").unwrap());
static PUSHOVER_TOKEN: LazyLock<String> = LazyLock::new(|| env::var("PUSHOVER_TOKEN").unwrap());

const BODY_LIMIT: usize = 20 * 1024 * 1024; // 20 MiB

#[tokio::main]
async fn main() {
    let app = Router::new().route(
        "/",
        post(|body: Body| async {
            let compressed_bytes = to_bytes(body, BODY_LIMIT).await.unwrap();
            let bytes = GzDecoder::new(&compressed_bytes[..]);

            for result in Deserializer::from_reader(bytes).into_iter().take(1) {
                let TaskInfo { index_uid, status, r#type, error, details, duration } = match result {
                    Ok(task) => task,
                    Err(e) => {
                        eprintln!("{e:?}");
                        continue;
                    }
                };

                let message_builder = MessageBuilder::new(&PUSHOVER_USER, &PUSHOVER_TOKEN, "");
                let message_builder = match (
                    index_uid,
                    error.as_ref().map(|e| &e.message),
                    details.as_ref().and_then(|d| d.received_documents),
                    details.as_ref().and_then(|d| d.indexed_documents),
                ) {
                    (Some(index_uid), Some(error_message), Some(received_documents), _) => {
                        message_builder
                            .set_title(&format!(
                                "Index {index_uid} {status} indexing {received_documents} documents in {duration:#}"
                            ))
                          .modify_message(&format!("{}: {error_message}", r#type))
                    }
                    (Some(index_uid), None, _, Some(indexed_documents)) => {
                        message_builder.set_title(&format!(
                            "Index {index_uid} {status} {indexed_documents} documents in {duration:#}"
                        ))
                          .modify_message(&r#type)
                    },
                    (None, _, _, _) => {
                        message_builder.set_title(&format!("Indexing {status} in {duration:#}"))
                          .modify_message(&r#type)
                    },
                    (Some(index_uid), _, _, _) => {
                        message_builder.set_title(&format!(
                            "Index {index_uid} {status} in {duration:#}"
                        ))
                          .modify_message(&r#type)
                    }
                };

                send_pushover_request(message_builder.build())
                    .await
                    .unwrap();
            }
        }),
    );

    // run our app with hyper, listening globally on port 3000
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskInfo {
    index_uid: Option<String>,
    status: String,
    r#type: String,
    error: Option<TaskError>,
    details: Option<TaskDetails>,
    duration: SignedDuration,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskError {
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskDetails {
    indexed_documents: Option<u64>,
    received_documents: Option<u64>,
}
