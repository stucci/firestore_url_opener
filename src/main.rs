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

async fn initialize_firestore(project_id: &str) -> Result<FirestoreDb, Box<dyn std::error::Error>> {
    let db = FirestoreDb::new(project_id).await?;
    info!("Connected to Firestore");
    Ok(db)
}

async fn initialize_listener(db: &FirestoreDb) -> Result<FirestoreListener<FirestoreDb, FirestoreTempFilesListenStateStorage>, Box<dyn std::error::Error>> {
    let listener = db
        .create_listener(FirestoreTempFilesListenStateStorage::new())
        .await?;
    Ok(listener)
}

fn handle_url(url: &str) {
    if let Ok(decoded_url) = percent_decode_str(url).decode_utf8() {
        info!("Opening decoded URL: {}", decoded_url);
        if let Err(e) = webbrowser::open(decoded_url.as_ref()) {
            error!("Failed to open URL in browser: {}", e);
        }
    } else {
        error!("Failed to decode URL: {}", url);
    }
}

fn handle_document_change(doc: &FirestoreDocument) {
    if let Ok(shared_url) = FirestoreDb::deserialize_doc_to::<SharedUrl>(doc) {
        info!("Received new URL: {}", shared_url.url);
        handle_url(&shared_url.url);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    dotenv::dotenv().ok();

    // Get project ID from environment
    let project_id = env::var("PROJECT_ID")
        .expect("PROJECT_ID environment variable must be set");

    // Initialize Firestore and listener
    let db = initialize_firestore(&project_id).await?;
    let mut listener = initialize_listener(&db).await?;

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
                        handle_document_change(doc);
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
