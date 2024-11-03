use mongodb::{Client, options::ClientOptions, Collection};
use dotenv::dotenv;
use std::env;

pub mod models;
use models::Entry;

pub async fn get_db_collection() -> mongodb::error::Result<Collection<Entry>> {
    dotenv().ok();
    let database_url = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let database_name = env::var("DB_NAME").expect("DB_NAME must be set.");

    let client_options = ClientOptions::parse(&database_url).await.expect("Cannot get the client option");
    let client = Client::with_options(client_options).expect("Cannot get the client");
    let db = client.database(&database_name);

    let collection = db.collection::<Entry>("entries");
    Ok(collection)
}
