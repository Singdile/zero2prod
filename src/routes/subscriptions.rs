//! src/routes/subscriptions.rs
//!
use chrono::Utc;
use uuid::Uuid;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;
#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

///总是返回200 ok
pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> HttpResponse {
    let res =
    sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at) VALUES ($1, $2, $3, $4)"#, Uuid::new_v4(), form.email, form.name, Utc::now()
    ).execute(pool.get_ref())
    .await;

    match res {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            println!("Failed to execute query: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }

}
