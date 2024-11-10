use actix_cors::Cors;
use actix_multipart::Multipart;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chrono::Datelike;
use futures::StreamExt;
use mongodb::bson::doc;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;

pub mod models;
use models::*;

mod helpers;
use helpers::*;

#[derive(Deserialize)]
struct GetEntriesParams {
    limit: i64,
    offset: u64,
}

#[derive(Deserialize)]
struct AddEntryParams {
    log: Option<String>,
}

async fn add_entry(params: web::Query<AddEntryParams>, mut payload: Multipart) -> impl Responder {
    // Prepare a vacant entry but accept 'log=' query for simplicity.
    let mut entry = Entry {
        id: None,
        content: params.log.clone().unwrap_or_else(String::new),
        created_at: chrono::Utc::now(),
        attachments: None,
    };

    // Empty attachments
    let mut attachments = Vec::new();

    while let Some(Ok(mut field)) = payload.next().await {
        use std::path::Path;

        let content_disposition = field.content_disposition().clone();
        let field_name = content_disposition.get_name().unwrap_or_default();

        // Content (can be replaced with '?log=' query)
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
            let tstamp = entry.created_at.naive_utc();
            let directory_path = format!(
                "./attachments/{:04}/{:02}/{:02}",
                tstamp.year(),
                tstamp.month(),
                tstamp.day()
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
    if !attachments.is_empty() {
        entry.attachments = Some(attachments);
    }

    if entry.content.is_empty() && entry.attachments.is_none() {
        // No entries at all
        return HttpResponse::InternalServerError().body("Error no log nor attachment entry");
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

    // Clear the download requests hash map asynchronously
    clean_expired_download_request().await;

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
                // Make download hash
                let download_token = generate_temporary_download_url(&attachment).await;
                // Don't pass the saved path to the frontend!
                attachments_response.push(AttachmentResponse {
                    id: attachment.id,
                    mime: attachment.mime.clone(),
                    original_name: attachment.original_name,
                    download_token,
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

async fn greet() -> impl Responder {
    HttpResponse::Ok().body("Hello elogbook backend! Please use HTTP requests.")
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
