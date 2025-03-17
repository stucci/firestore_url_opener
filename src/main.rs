use firestore::*;
use serde::{Deserialize, Serialize};
use std::env;
use tokio::signal;
use webbrowser;
use log::{info, error};
use chrono::prelude::*;
use percent_encoding::percent_decode_str;
use chrono::Duration;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SharedUrl {
    #[serde(alias = "_firestore_id")]
    doc_id: Option<String>,
    url: String,
    #[serde(with = "firestore::serialize_as_timestamp")]
    timestamp: DateTime<Utc>,
    #[serde(with = "firestore::serialize_as_optional_timestamp", default)]
    expired_at: Option<DateTime<Utc>>,
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

async fn handle_document_change(db: &FirestoreDb, doc: &FirestoreDocument) {
    if let Ok(shared_url) = FirestoreDb::deserialize_doc_to::<SharedUrl>(doc) {
        info!("Received new URL: {}", shared_url.url);
        handle_url(&shared_url.url);

        // Calculate expired_at timestamp
        let expired_at = Utc::now() + Duration::days(3);

        // Create a struct for the update operation to properly handle timestamps
        #[derive(Debug, Clone, Deserialize, Serialize)]
        struct SharedUrlUpdate {
            url: String,
            #[serde(with = "firestore::serialize_as_timestamp")]
            timestamp: DateTime<Utc>,
            #[serde(with = "firestore::serialize_as_timestamp")]
            expired_at: DateTime<Utc>,
        }

        let update_data = SharedUrlUpdate {
            url: shared_url.url.clone(),
            timestamp: shared_url.timestamp,
            expired_at: expired_at,
        };

        // Update the document with all necessary fields
        if let Some(doc_id) = &shared_url.doc_id {
            let update_result = db
                .fluent()
                .update()
                .in_col("shared_urls")
                .document_id(doc_id)
                .object(&update_data)
                .execute::<SharedUrl>()
                .await;

            match update_result {
                Ok(_) => info!("Document updated with expired_at"),
                Err(e) => error!("Failed to update document with expired_at: {}", e),
            }
        }
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
        .start(move |event| {
            let db = db.clone();  // Clone db to move it into the closure
            async move {
                match event {
                    FirestoreListenEvent::DocumentChange(doc_change) => {
                        if let Some(doc) = &doc_change.document {
                            // Check if 'expired_at' field is already present
                            if !doc.fields.contains_key("expired_at") {
                                handle_document_change(&db, doc).await;
                            }
                        }
                    }
                    _ => {
                        info!("Received other event: {:?}", event);
                    }
                }
                Ok(())
            }
        })
        .await?;

    // Wait for Ctrl+C
    signal::ctrl_c().await?;
    info!("Received interrupt signal, shutting down...");
    listener.shutdown().await?;

    Ok(())
} 
