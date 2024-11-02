use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use elogbook::establish_connection;
use elogbook::models::NewEntry; // models を正しくインポート
use serde::Deserialize;
use actix_cors::Cors;

#[derive(Deserialize)]
struct LogEntry {
    content: String,
}

#[derive(Deserialize)]
struct GetEntriesParams {
    limit:  i64,
    offset: i64,
}

pub async fn add_entry(entry: web::Json<LogEntry>) -> impl Responder {
    use elogbook::schema::entries::dsl::*;
    use diesel::prelude::*;

    let connection = establish_connection();
    let new_entry = NewEntry {
        content: &entry.content,
    };
    
    diesel::insert_into(entries)
                .values(&new_entry)
                .execute(&connection)
                .expect("Error saving new entry");
    
    HttpResponse::Ok().body("Entry added to database.")
}

pub async fn get_entries(params: web::Query<GetEntriesParams>) -> impl Responder{
    use elogbook::schema::entries::dsl::*;
    use diesel::prelude::*;

    let connection = establish_connection();
    let results = entries
                    .order_by(id.desc())
                    .limit(params.limit)
                    .offset(params.offset)
                    .load::<(i32, String, chrono::DateTime<chrono::Utc>)>(&connection)
                    .expect("Error loading entries");
    //println!("{:?}", results);
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
