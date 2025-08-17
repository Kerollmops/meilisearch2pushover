use std::{env, ops::Add, sync::LazyLock};

use axum::{
    Router,
    body::{Body, to_bytes},
    routing::post,
};
use flate2::bufread::GzDecoder;
use jiff::SignedDuration;
use pushover_rs::{MessageBuilder, send_pushover_request};
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

            let result = Deserializer::from_reader(bytes).into_iter().reduce(|acc, result| {
                match (acc, result) {
                    (Ok(acc), Ok(info)) => Ok(acc + info),
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            });

            let TaskInfo { index_uid, status, r#type, error, details, duration } = match result {
                Some(Ok(task)) => task,
                Some(Err(e)) => {
                    eprintln!("{e:?}");
                    return;
                },
                None => return,
            };

                let duration = duration.round(jiff::Unit::Second).unwrap_or(duration);
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
                                "Index {index_uid} {status}"
                            ))
                          .modify_message(&format!("A {} {status} indexing {received_documents} documents in {duration:#}: {error_message}", r#type))
                    }
                    (Some(index_uid), None, _, Some(indexed_documents)) => {
                        let speed = indexed_documents as f64 / (duration.as_millis() as f64 / 1000.0);
                        message_builder.set_title(&format!(
                            "Index {index_uid} {status}"
                        ))
                          .modify_message(&format!("A {} {status} indexing {indexed_documents} documents in {duration:#} ({speed:.2} docs/s)", r#type))
                    },
                    (None, _, _, _) => {
                        message_builder.set_title(&format!("Task {status}"))
                          .modify_message(&format!("A {} {status} in {duration:#}", r#type))
                    },
                    (Some(index_uid), _, _, _) => {
                        message_builder.set_title(&format!(
                            "Index {index_uid} {status}"
                        ))
                          .modify_message(&format!("A {} {status} indexing in {duration:#}", r#type))
                    }
                };

                send_pushover_request(message_builder.build())
                    .await
                    .unwrap();
        })
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

impl Add for TaskInfo {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        TaskInfo {
            index_uid: self.index_uid.or(rhs.index_uid),
            status: self.status,
            r#type: self.r#type,
            error: self.error.or(rhs.error),
            details: match (self.details, rhs.details) {
                (Some(a), Some(b)) => Some(a + b),
                (Some(x), _) | (_, Some(x)) => Some(x),
                (None, None) => None,
            },
            duration: self.duration,
        }
    }
}

impl Add for TaskDetails {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        TaskDetails {
            indexed_documents: match (self.indexed_documents, rhs.indexed_documents) {
                (Some(a), Some(b)) => Some(a + b),
                (Some(x), _) | (_, Some(x)) => Some(x),
                (None, None) => None,
            },
            received_documents: match (self.received_documents, rhs.received_documents) {
                (Some(a), Some(b)) => Some(a + b),
                (Some(x), _) | (_, Some(x)) => Some(x),
                (None, None) => None,
            },
        }
    }
}
