use crate::models::*;

use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client, Collection};
use std::env;

use actix_files::NamedFile;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use chrono::{Duration, Utc};
use lazy_static::lazy_static;
use rand::{distributions::Alphanumeric, Rng};
use std::collections::HashMap;
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
    static ref DOWNLOAD_REQUESTS: Mutex<HashMap<String, DownloadRequest>> =
        Mutex::new(HashMap::new());
}

// Clean up the expired entries in the hash map
pub async fn clean_expired_download_request() {
    let mut requests = DOWNLOAD_REQUESTS.lock().await;
    requests.retain(|_, req| Utc::now() < req.expires_at);
}

// Generate a temporary token for download
pub async fn generate_temporary_download_url(attachment: &Attachment) -> String {
    let token: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();

    let expires_at = Utc::now() + Duration::minutes(1); // available 1min

    let download_request = DownloadRequest {
        token: token.clone(),
        file_path: attachment.saved_path.clone(),
        original_name: attachment.original_name.clone(),
        expires_at: expires_at,
    };

    DOWNLOAD_REQUESTS
        .lock()
        .await
        .insert(token.clone(), download_request);

    // Return the token = hash
    token
}

pub async fn download_file(
    req: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let token = path.into_inner();
    let mut requests = DOWNLOAD_REQUESTS.lock().await;

    if let Some(request) = requests.remove(&token) {
        if Utc::now() < request.expires_at {
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
    }

    Ok(HttpResponse::NotFound().body("Link expired or invalid"))
}
