use reqwest::Client;
use serde_json::Value;
use tokio::time::{sleep, Duration};
use webbrowser;
use url::Url;
use percent_encoding::percent_decode_str;

const FIRESTORE_URL: &str = "https://firestore.googleapis.com/v1/projects/push-share-452f8/databases/(default)";

#[tokio::main]
async fn main() {
    let client = Client::new();
    let mut last_seen_url: Option<String> = None;
    let mut is_first_run = true;

    loop {
        match fetch_latest_url(&client).await {
            Ok(Some(url)) => {
                if last_seen_url.as_ref() != Some(&url) {
                    println!("New URL received: {}", url);
                    if !is_first_run {
                        if let Some(decoded) = percent_decode_str(&url).decode_utf8().ok() {
                            if let Ok(parsed_url) = Url::parse(&decoded.to_string()) {
                                if webbrowser::open(parsed_url.as_str()).is_ok() {
                                    println!("Opened in browser.");
                                } else {
                                    println!("Failed to open browser.");
                                }
                            } else {
                                println!("Failed to parse URL.");
                            }
                        } else {
                            println!("Failed to decode URL.");
                        }
                    }
                    last_seen_url = Some(url);
                    is_first_run = false;
                }
            }
            Ok(None) => println!("No new URLs."),
            Err(e) => eprintln!("Error fetching URL: {:?}", e),
        }

        // 5秒ごとにチェック
        sleep(Duration::from_secs(5)).await;
    }
}

async fn fetch_latest_url(client: &Client) -> Result<Option<String>, reqwest::Error> {
    let query = serde_json::json!({
        "structuredQuery": {
            "from": [{
                "collectionId": "shared_urls"
            }],
            "orderBy": [{
                "field": {
                    "fieldPath": "timestamp"
                },
                "direction": "DESCENDING"
            }],
            "limit": 1
        }
    });

    let url = format!("{}/documents:runQuery", FIRESTORE_URL);
    let response = client
        .post(&url)
        .json(&query)
        .send()
        .await?;
    let body: Value = response.json().await?;

    // レスポンスの内容を全て表示
    println!("Firestore Response: {}", serde_json::to_string_pretty(&body).unwrap());

    if let Some(documents) = body.as_array() {
        println!("Found {} documents", documents.len());
        for (i, doc) in documents.iter().enumerate() {
            println!("Document {}: {}", i, serde_json::to_string_pretty(doc).unwrap());
        }
        
        if let Some(last_doc) = documents.first() {
            if let Some(fields) = last_doc.get("document").and_then(|d| d.get("fields")) {
                println!("Fields: {}", serde_json::to_string_pretty(fields).unwrap());
                if let Some(url) = fields.get("url").and_then(|u| u.get("stringValue")).and_then(|v| v.as_str()) {
                    return Ok(Some(url.to_string()));
                }
            }
        }
    }

    Ok(None)
}
