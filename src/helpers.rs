use crate::models::*;

use actix_files::NamedFile;
use actix_web::{web, Error, HttpRequest, HttpResponse, Responder};
use chrono::{Duration, Utc};
use dotenv::dotenv;
use lazy_static::lazy_static;
use mongodb::{options::ClientOptions, Client, Collection};
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;
use std::collections::{hash_map, HashMap};
use std::env;
use tokio::sync::Mutex;

pub async fn get_db_collection() -> mongodb::error::Result<Collection<Entry>> {
    dotenv().ok();
    let database_url = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let database_name = env::var("DB_NAME").expect("DB_NAME must be set.");

    let client_options = ClientOptions::parse(&database_url)
        .await
        .expect("Cannot get the client option");
    let client = Client::with_options(client_options).expect("Cannot get the client");
    let db = client.database(&database_name);

    let collection = db.collection::<Entry>("entries");
    Ok(collection)
}

lazy_static! {
    static ref DOWNLOAD_REQUESTS: Mutex<HashMap<String, DownloadClient>> =
        Mutex::new(HashMap::new());
}

// Clean up the expired entries in the hash map
pub async fn clean_expired_download_request() {
    let mut clients = DOWNLOAD_REQUESTS.lock().await;
    clients.retain(|_, client| Utc::now() < client.expires_at);
}

#[derive(Deserialize)]
pub struct ExtendDownloadParams {
    pub client: String,
}
// Extend the download hash lifetime for the given client
pub async fn extend_download_lifetime(params: web::Query<ExtendDownloadParams>) -> impl Responder {
    let mut clients = DOWNLOAD_REQUESTS.lock().await;
    if let hash_map::Entry::Occupied(entry) = clients.entry(params.client.clone()) {
        // Extend the expire date by 5 minutes
        let client = entry.into_mut();
        client.expires_at = Utc::now() + Duration::minutes(5);
    }
    HttpResponse::Ok()
}

// Generate a temporary token for download
pub async fn generate_temporary_download_url(
    client_hash: String,
    attachment: &Attachment,
) -> String {
    let token: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();

    let download_request = DownloadRequest {
        file_path: attachment.saved_path.clone(),
        original_name: attachment.original_name.clone(),
    };

    // Get the DownloadClient map
    let mut clients = DOWNLOAD_REQUESTS.lock().await;
    let client = clients.entry(client_hash).or_insert(DownloadClient::new());
    // Extend the expire date or newly set
    client.expires_at = Utc::now() + Duration::minutes(5);

    // Insert to this client
    client.requests.insert(token.clone(), download_request);

    // Return the token = hash
    token
}

#[derive(Deserialize)]
pub struct DownloadParams {
    pub client: String,
    pub token: String,
}

pub async fn download_file(
    req: HttpRequest,
    params: web::Query<DownloadParams>,
) -> Result<HttpResponse, Error> {
    // Check the client hash
    let client_hash = &params.client;
    let mut clients = DOWNLOAD_REQUESTS.lock().await;
    let client = match clients.entry(client_hash.clone()) {
        // No client recognised
        hash_map::Entry::Vacant(_) => {
            return Ok(
                HttpResponse::NotFound().body(format!("Unrecognised client: {}", client_hash))
            );
        }
        // If found, check the expiration
        hash_map::Entry::Occupied(entry) => {
            if Utc::now() >= entry.get().expires_at {
                return Ok(
                    HttpResponse::BadRequest().body(format!("Expired client: {}", client_hash))
                );
            }
            // Still alive
            entry.into_mut()
        }
    };

    let token = &params.token;
    let requests = &mut client.requests;
    if let Some(request) = requests.remove(token) {
        let named_file = NamedFile::open_async(&request.file_path).await?;

        // Return the file with a meta data = the original name
        return Ok(named_file
            .use_last_modified(true)
            .set_content_disposition(actix_web::http::header::ContentDisposition {
                disposition: actix_web::http::header::DispositionType::Attachment,
                parameters: vec![actix_web::http::header::DispositionParam::Filename(
                    request.original_name.clone(),
                )],
            })
            .into_response(&req));
    }

    Ok(HttpResponse::NotFound().body(format!("Unrecognised token: {}", token)))
}
