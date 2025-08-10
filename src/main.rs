use std::{env, sync::LazyLock};

use axum::{
    body::{to_bytes, Body},
    routing::post,
    Router,
};
use flate2::bufread::GzDecoder;
use jiff::{SignedDuration, Unit};
use pushover_rs::{send_pushover_request, MessageBuilder};
use serde_json::{de::Deserializer, Value};
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
                let value: Value = match result {
                    Ok(value) => value,
                    Err(e) => {
                        eprintln!("{e:?}");
                        continue;
                    }
                };

                let index_uid = value.get("indexUid").and_then(|v| v.as_str());
                let status = value.get("status").unwrap().as_str().unwrap();
                let r#type = value.get("type").unwrap().as_str().unwrap();
                let error_message = value.get("error.message").and_then(|v| v.as_str());
                let indexed_documents = value
                    .get("details.indexedDocuments")
                    .and_then(|v| v.as_u64());
                let received_documents = value
                    .get("details.receivedDocuments")
                    .and_then(|v| v.as_u64());
                let duration = value
                    .get("duration")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .parse::<SignedDuration>()
                    .unwrap()
                    .round(Unit::Second)
                    .unwrap();

                let message_builder = MessageBuilder::new(&PUSHOVER_USER, &PUSHOVER_TOKEN, "");
                let message_builder = match (
                    index_uid,
                    error_message,
                    received_documents,
                    indexed_documents,
                ) {
                    (Some(index_uid), Some(error_message), Some(received_documents), _) => {
                        message_builder
                            .set_title(&format!(
                                "Index {index_uid} failed indexing {received_documents} documents in {duration}"
                            ))
                          .modify_message(&format!("{}: {error_message}", r#type))
                    }
                    (Some(index_uid), None, _, Some(indexed_documents)) => {
                        message_builder.set_title(&format!(
                            "Index {index_uid} finished {status} {indexed_documents} documents in {duration}"
                        ))
                          .modify_message(r#type)
                    },
                    (None, _, _, _) => {
                        message_builder.set_title(&format!("Indexing finished in {duration}"))
                          .modify_message(r#type)
                    },
                    (Some(index_uid), _, _, _) => {
                        message_builder.set_title(&format!(
                            "Index {index_uid} finished {status} in {duration}"
                        ))
                          .modify_message(r#type)
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
