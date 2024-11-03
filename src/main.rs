use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use actix_cors::Cors;
use mongodb::bson::doc;

use elogbook::get_db_collection;
use elogbook::models::{Entry, EntryResponse};

#[derive(Deserialize)]
struct LogEntry {
    content: String,
}

#[derive(Deserialize)]
struct GetEntriesParams {
    limit:  i64,
    offset: u64,
}

async fn add_entry(entry: web::Json<LogEntry>) -> impl Responder {

    let collection = get_db_collection().await.expect("Failed to connect to the DB.");
    
    let new_entry = Entry {
        id:         None,
        content:    entry.content.clone(),
        created_at: chrono::Utc::now(),
    };

    // Insert this log
    collection.insert_one(new_entry, None).await.expect("Error saving new entry");
    
    HttpResponse::Ok().body("Entry added to database.")
}

async fn get_entries(params: web::Query<GetEntriesParams>) -> impl Responder{
    use futures_util::stream::TryStreamExt;  // TryStreamExtのインポート

    let collection = get_db_collection().await.expect("Failed to connect to the DB.");

    let find_options = mongodb::options::FindOptions::builder()
        .sort(mongodb::bson::doc! {"_id": -1})
        .limit(params.limit)
        .skip(params.offset)
        .build();

    let mut cursor = collection.find(None, find_options).await.expect("Error finding entries.");
    let mut results = Vec::new();

    while let Some(entry) = cursor.try_next().await.expect("Error parsing entry") {
        let entry_response = EntryResponse {
            id:         entry.id.map(|oid| oid.to_hex()),  // ObjectIdをStringに変換
            content:    entry.content,
            created_at: entry.created_at,
        };
        results.push(entry_response);
    }

    HttpResponse::Ok().json(results)
}

async fn greet() -> impl Responder{
    HttpResponse::Ok().body("Hello World!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(
                Cors::default()
                    .allow_any_origin() // 必要であれば、特定のオリジンに限定することも可能
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600),
            )   
            .route("/", web::get().to(greet))
            .route("/add_entry", web::post().to(add_entry))
            .route("/get_entries", web::get().to(get_entries))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
