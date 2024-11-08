use actix_cors::Cors;
use actix_multipart::Multipart;
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use chrono::{Datelike, Duration, Utc};
use futures::StreamExt;
use mongodb::bson::doc;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;

use actix_files::NamedFile;
use lazy_static::lazy_static;
use rand::{distributions::Alphanumeric, Rng};
use std::collections::HashMap;
use tokio::sync::Mutex;

use elogbook::get_db_collection;
use elogbook::models::*;

#[derive(Deserialize)]
struct GetEntriesParams {
    limit: i64,
    offset: u64,
}

async fn add_entry(mut payload: Multipart) -> impl Responder {
    // Prepare a vacant entry
    let mut entry = Entry {
        id: None,
        content: String::new(),
        created_at: chrono::Utc::now(),
        attachments: None,
    };
    // Empty attachments
    let mut attachments = Vec::new();

    while let Some(item) = payload.next().await {
        use std::path::Path;

        let mut field = item.expect("Error processing multipart field");

        let content_disposition = field.content_disposition().clone();
        let field_name = content_disposition.get_name().unwrap_or_default();

        // Content
        if field_name == "content" {
            while let Some(chunk) = field.next().await {
                entry.content.push_str(
                    std::str::from_utf8(&chunk.expect("Error getting a log content")).unwrap(),
                );
            }
        }
        // Attachments
        else if field_name == "file" {
            let mime_type = field.content_type().to_string();
            let original_filename = content_disposition.get_filename().unwrap_or("tmpfile");
            let directory_path = format!(
                "./attachments/{:04}/{:02}/{:02}",
                entry.created_at.naive_utc().year(),
                entry.created_at.naive_utc().month(),
                entry.created_at.naive_utc().day()
            );

            // Make a unique (hashed) name
            let mut hasher = Sha256::new();
            hasher.update(entry.created_at.to_rfc3339().as_bytes());
            hasher.update(original_filename);
            hasher.update(attachments.len().to_string());
            let hash_result = format!("{:x}", hasher.finalize());
            let mut hashed_filename = format!("{}/{}", directory_path, hash_result);
            // Extention?
            if let Some(extention) = Path::new(original_filename).extension() {
                if let Some(ext_str) = extention.to_str() {
                    hashed_filename += &format!(".{}", ext_str);
                }
            }

            std::fs::create_dir_all(&directory_path).expect("Cannot make directory");
            let mut f = File::create(&hashed_filename)
                .expect(&format!("Error creating file: {}", &hashed_filename));

            while let Some(chunk) = field.next().await {
                let data = match chunk {
                    Ok(d) => d,
                    Err(e) => {
                        return HttpResponse::InternalServerError()
                            .body(format!("Error reading data: {}", e))
                    }
                };
                if let Err(e) = f.write_all(&data) {
                    return HttpResponse::InternalServerError()
                        .body(format!("Error writing to file: {}", e));
                }
            }

            // Create the attachment entry
            let attachment = Attachment {
                id: attachments.len() as u32 + 1,
                saved_path: hashed_filename.clone(),
                original_name: original_filename.to_string(),
                mime: mime_type,
            };
            attachments.push(attachment);
        }
    }
    if 0 < attachments.len() {
        entry.attachments = Some(attachments);
    }

    // Insert this log entry
    let collection = get_db_collection()
        .await
        .expect("Failed to connect to the DB.");
    collection
        .insert_one(entry, None)
        .await
        .expect("Error saving new entry");

    HttpResponse::Ok().body("Entry added to database.")
}

async fn get_entries(params: web::Query<GetEntriesParams>) -> impl Responder {
    use futures_util::stream::TryStreamExt;

    let collection = get_db_collection()
        .await
        .expect("Failed to connect to the DB.");

    let find_options = mongodb::options::FindOptions::builder()
        .sort(mongodb::bson::doc! {"_id": -1})
        .limit(params.limit)
        .skip(params.offset)
        .build();

    let mut cursor = collection
        .find(None, find_options)
        .await
        .expect("Error finding entries.");
    let mut results = Vec::new();

    while let Some(entry) = cursor.try_next().await.expect("Error parsing entry") {
        let mut attachments_response = vec![];

        // Process attachments
        if let Some(attachments) = entry.attachments {
            for attachment in attachments {
                // Make download URL
                let download_url = generate_temporary_download_url(&attachment).await;
                // Don't pass the saved path to the frontend!
                attachments_response.push(AttachmentResponse {
                    id: attachment.id,
                    mime: attachment.mime.clone(),
                    original_name: attachment.original_name,
                    download_url: download_url,
                });
            }
        }

        let entry_response = EntryResponse {
            id: entry.id.map(|oid| oid.to_hex()),
            content: entry.content,
            created_at: entry.created_at,
            attachments: attachments_response,
        };
        results.push(entry_response);
    }

    HttpResponse::Ok().json(results)
}

lazy_static! {
    static ref DOWNLOAD_REQUESTS: Mutex<HashMap<String, DownloadRequest>> =
        Mutex::new(HashMap::new());
}

// 一時的なダウンロードURLを生成
async fn generate_temporary_download_url(attachment: &Attachment) -> String {
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
    format!("download/{}", token)
}

async fn download_file(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let token = path.into_inner();
    let requests = DOWNLOAD_REQUESTS.lock().await;

    if let Some(request) = requests.get(&token) {
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

async fn greet() -> impl Responder {
    HttpResponse::Ok().body("Hello World!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    //use actix_files as fs;

    HttpServer::new(|| {
        App::new()
            .wrap(
                Cors::default()
                    .allow_any_origin() // 必要であれば、特定のオリジンに限定することも可能
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600),
            )
            //.service(fs::Files::new("/attachments", "./attachments").show_files_listing())
            .route("/", web::get().to(greet))
            .route("/add_entry", web::post().to(add_entry))
            .route("/get_entries", web::get().to(get_entries))
            .route("/download/{token}", web::get().to(download_file))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
