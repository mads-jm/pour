---
tags:
  - reference
  - rust
  - http
aliases:
  - reqwest
date created: Tuesday, March 31st 2026, 12:14:41 am
date modified: Tuesday, March 31st 2026, 3:25:21 am
---

# Reqwest - HTTP Client Reference

> __Source:__ <https://docs.rs/reqwest/latest/reqwest/>
> __Crate:__ `reqwest` (with `json` feature)

## Role in Pour

Reqwest is used to communicate with the [[obsidian-local-rest-api|Obsidian Local REST API]] over HTTP/HTTPS on localhost.

## Client Setup

```rust
use reqwest::Client;

// Basic client
let client = Client::new();

// With custom config (e.g., accept self-signed certs)
let client = Client::builder()
    .danger_accept_invalid_certs(true)  // needed for Obsidian's self-signed cert
    .timeout(std::time::Duration::from_secs(5))
    .build()?;
```

## Making Requests

### GET

```rust
let resp = client.get("https://127.0.0.1:27124/vault/path/to/file.md")
    .bearer_auth("your-api-key")
    .send()
    .await?;

let body = resp.text().await?;
```

### PUT (create/overwrite file)

```rust
let resp = client.put("https://127.0.0.1:27124/vault/path/to/note.md")
    .bearer_auth("your-api-key")
    .header("Content-Type", "text/markdown")
    .body(markdown_content)
    .send()
    .await?;
```

### POST (append to file)

```rust
let resp = client.post("https://127.0.0.1:27124/vault/path/to/note.md")
    .bearer_auth("your-api-key")
    .header("Content-Type", "text/markdown")
    .body(content_to_append)
    .send()
    .await?;
```

### PATCH (surgical edit)

```rust
let resp = client.patch("https://127.0.0.1:27124/vault/path/to/note.md")
    .bearer_auth("your-api-key")
    .header("Operation", "append")
    .header("Target-Type", "heading")
    .header("Target", "Brain Dump")
    .header("Content-Type", "text/markdown")
    .body("- New thought here\n")
    .send()
    .await?;
```

### JSON requests/responses

```rust
// Send JSON
let resp = client.post(url)
    .bearer_auth(api_key)
    .json(&serde_json::json!({"key": "value"}))
    .send()
    .await?;

// Parse JSON response
let data: serde_json::Value = resp.json().await?;
```

## Response Handling

```rust
let resp = client.get(url).bearer_auth(key).send().await?;

// Check status
if resp.status().is_success() {
    let body = resp.text().await?;
} else {
    let status = resp.status();
    let error_body = resp.text().await?;
    // handle error
}
```

## Connection Check Pattern (for Pour)

```rust
/// Check if Obsidian REST API is available
async fn check_api(client: &Client, port: u16) -> bool {
    client.get(format!("https://127.0.0.1:{}/", port))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
```

## Directory Listing (for Dynamic selects)

```rust
let resp = client.get("https://127.0.0.1:27124/vault/02-Logbook/Beans/")
    .bearer_auth(api_key)
    .send()
    .await?;

let listing: serde_json::Value = resp.json().await?;
// listing["files"] is an array of {path, stat} objects
```
