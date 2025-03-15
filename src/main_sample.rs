use firestore::*;
use serde::{Deserialize, Serialize};
use std::env;
use tokio::signal;
use webbrowser;
use log::{info, error};
use chrono::prelude::*;
use percent_encoding::percent_decode_str;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SharedUrl {
    #[serde(alias = "_firestore_id")]
    doc_id: Option<String>,
    url: String,
    #[serde(with = "firestore::serialize_as_timestamp")]
    timestamp: DateTime<Utc>,
}

const TARGET_ID: FirestoreListenerTarget = FirestoreListenerTarget::new(42u32);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    dotenv::dotenv().ok();

    // Get project ID from environment
    let project_id = env::var("PROJECT_ID")
        .expect("PROJECT_ID environment variable must be set");

    // Initialize Firestore database
    let db = FirestoreDb::new(&project_id).await?;
    info!("Connected to Firestore");

    // Create listener with temporary file storage
    let mut listener = db
        .create_listener(
            FirestoreTempFilesListenStateStorage::new(),
        )
        .await?;

    let collection_name = "shared_urls";

    // Start listening for changes using fluent API
    db.fluent()
        .select()
        .from(collection_name)
        .listen()
        .add_target(TARGET_ID, &mut listener)?;

    info!("Starting to listen for changes in collection: {}", collection_name);

    // Start the listener with a callback
    listener
        .start(|event| async move {
            match event {
                FirestoreListenEvent::DocumentChange(doc_change) => {
                    if let Some(doc) = &doc_change.document {
                        if let Ok(shared_url) = FirestoreDb::deserialize_doc_to::<SharedUrl>(doc) {
                            info!("Received new URL: {}", shared_url.url);
                            // Decode the URL before opening
                            if let Ok(decoded_url) = percent_decode_str(&shared_url.url).decode_utf8() {
                                info!("Opening decoded URL: {}", decoded_url);
                                if let Err(e) = webbrowser::open(decoded_url.as_ref()) {
                                    error!("Failed to open URL in browser: {}", e);
                                }
                            } else {
                                error!("Failed to decode URL: {}", shared_url.url);
                            }
                        }
                    }
                }
                _ => {
                    info!("Received other event: {:?}", event);
                }
            }
            Ok(())
        })
        .await?;

    // Wait for Ctrl+C
    signal::ctrl_c().await?;
    info!("Received interrupt signal, shutting down...");
    listener.shutdown().await?;

    Ok(())
} 
