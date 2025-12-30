//! src/routes/subscriptions.rs
//!
use actix_web::{HttpResponse, web};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

///邮件订阅服务,总是返回200 ok
///为函数专注于业务逻辑的处理，将日志等“插桩”信息交给过程宏,值得注意的是在默认的情况下面，tracing::instrument 会将所有传递给函数的参数都放入到跨度的上下文中，必须指明日志中不需要的输入
///时刻注意这个不需要的日志信息是非常危险的，可能会导致信息泄漏,采用secrecy::Secret 来避免这个问题
#[tracing::instrument(name = "Adding a new subscriber", skip(form,pool), fields (subscriber_email = %form.email, subscriber_name = %form.name))]
pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> HttpResponse {
    match insert_subscriber(&form, &pool).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_e) => HttpResponse::InternalServerError().finish(),
    }
}

//将插入订阅者信息的操作单独为一个函数，并为该函数“插桩”
#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(form, pool)
)]
pub async fn insert_subscriber(form: &FormData, pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at) VALUES ($1, $2, $3, $4)"#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed tp execute query: {:?}", e);
        e
    })?;

    Ok(())
}
