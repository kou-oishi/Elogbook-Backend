// @generated automatically by Diesel CLI.

diesel::table! {
    entries (id) {
        id -> Int4,
        content -> Text,
        timestamp -> Timestamptz,
    }
}
