use std::env;

use axum::{routing::get, Router};
use pushover_rs::{send_pushover_request, MessageBuilder};

mod tasks;

#[tokio::main]
async fn main() {
    let app = Router::new().route(
        "/",
        get(|| async {
            let pushover_user = env::var("PUSHOVER_USER").unwrap();
            let pushover_token = env::var("PUSHOVER_TOKEN").unwrap();

            let message = MessageBuilder::new(&pushover_user, &pushover_token, "Example message")
                .set_title("Example push notification sent through Pushover API")
                .set_url("https://pushover.net/", Some("Pushover"))
                .build();

            send_pushover_request(message).await.unwrap();
        }),
    );

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
