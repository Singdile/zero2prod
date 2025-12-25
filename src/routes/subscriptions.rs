//! src/routes/subscriptions.rs
//!
use actix_web::{HttpResponse, web};
use chrono::Utc;
use sqlx::PgPool;
use tracing::Instrument;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

///总是返回200 ok
pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> HttpResponse {
    let request_id = Uuid::new_v4();

    //创建一个info 级别的跨度
    let request_pan = tracing::info_span!("Adding a new subscriber.", %request_id, subscriber_email = %form.email, subscriber_name = %form.name);

    let _request_span_guard = request_pan.enter(); //激活跨度

    //创建一个info 级别的跨度，由于前面有一个激活的跨度，所以这里的跨度自动成为了字跨度
    let query_span = tracing::info_span!("Saving new subscriber details in the database");

    let res = sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at) VALUES ($1, $2, $3, $4)"#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(pool.get_ref())
    .instrument(query_span) //将这个执行任务的所有过程都记录在query_span这个跨度,当future被轮询的时候，自动进入Span;当future挂起的时候，自动推出span
    .await;

    match res {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e); //查询失败，会返回日志信息
            HttpResponse::InternalServerError().finish()
        }
    }
}
