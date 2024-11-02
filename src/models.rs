use diesel::prelude::*;
use diesel::Insertable;
use super::schema::entries;

#[derive(Insertable)]
#[table_name = "entries"]
pub struct NewEntry<'a> {
    pub content: &'a str,
}
