# Meilisearch2Pushover

A small HTTP server to serve as a [Meilisearch webhook](https://www.meilisearch.com/docs/learn/async/task_webhook), which will notify task statuses via [Pushover](https://pushover.net).

## Usage

```bash
export PUSHOVER_USER=your-pushover-user-key
export PUSHOVER_TOKEN=your-pushover-app-token
cargo run --release
```