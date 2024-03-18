use std::{env, str::FromStr};

use axum::{
    body::{to_bytes, Body},
    routing::post,
    Router,
};
use flate2::bufread::GzDecoder;
use pushover_rs::{send_pushover_request, MessageBuilder};
use serde_json::{de::Deserializer, Value};
use std::time::Duration;
use tokio::net::TcpListener;

const BODY_LIMIT: usize = 20 * 1024 * 1024; // 20 MiB

#[tokio::main]
async fn main() {
    let app = Router::new().route(
        "/",
        post(|body: Body| async {
            let compressed_bytes = to_bytes(body, BODY_LIMIT).await.unwrap();
            let bytes = GzDecoder::new(&compressed_bytes[..]);

            let pushover_user = env::var("PUSHOVER_USER").unwrap();
            let pushover_token = env::var("PUSHOVER_TOKEN").unwrap();

            for result in Deserializer::from_reader(bytes).into_iter() {
                let value: Value = match result {
                    Ok(value) => value,
                    Err(e) => {
                        eprintln!("{e:?}");
                        continue;
                    }
                };

                let uid = value.get("uid").unwrap().as_u64().unwrap();
                let index_uid = value.get("indexUid").and_then(|v| v.as_str());
                let status = value.get("status").unwrap().as_str().unwrap();
                let duration: Duration =
                    iso8601::Duration::from_str(value.get("duration").unwrap().as_str().unwrap())
                        .unwrap()
                        .into();

                let message = match index_uid {
                    Some(index_uid) => format!(
                        "The task {uid} from {index_uid:?} {status} in processing in {duration:.02?}"
                    ),
                    None => format!(
                        "The task {uid} {status} in processing in {duration:.02?}"
                    ),
                };

                let message = MessageBuilder::new(&pushover_user, &pushover_token, &message)
                    .set_title("A task just finished processing")
                    .build();

                send_pushover_request(message).await.unwrap();
            }
        }),
    );

    // run our app with hyper, listening globally on port 3000
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
