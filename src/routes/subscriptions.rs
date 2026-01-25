//! src/routes/subscriptions.rs
//!
use std::ops::Deref;

use crate::email_client::EmailClient;
use crate::{domain::NewSubscriber, email_client};
use actix_web::{HttpResponse, web};
use chrono::Utc;
use sqlx::PgPool;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

///邮件订阅服务,总是返回200 ok
///为函数专注于业务逻辑的处理，将日志等“插桩”信息交给过程宏,值得注意的是在默认的情况下面，tracing::instrument 会将所有传递给函数的参数都放入到跨度的上下文中，必须指明日志中不需要的输入
///时刻注意这个不需要的日志信息是非常危险的，可能会导致信息泄漏,采用secrecy::Secret 来避免这个问题
#[tracing::instrument(name = "Adding a new subscriber", skip(form,pool,email_client), fields (subscriber_email = %form.email, subscriber_name = %form.name))]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> HttpResponse {
    let new_subscriber = match parse_subscriber(form.0) {
        Ok(subscriber) => subscriber,
        Err(_) => return HttpResponse::BadRequest().finish(), //表单数据错误，返回(400, BAD_REQUEST, "Bad Request");
    };

    //插入失败，返回500服务器内部错误
    if insert_subscriber(&new_subscriber, &pool).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    //插入成功，为新的订阅者发送一封确认邮件
    if send_confirmation_email(email_client.deref(), new_subscriber)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

///通过邮件服务商，像用户发送确认链接的邮件
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
) -> Result<(), reqwest::Error> {
    let confirmation_link = "https://my-api.com/subscriptions/confirm";
    let plain_body = &format!(
        "Welcome to our newsletter! \nVisit {} to confirm your subscription.",
        confirmation_link
    );

    let html_body = &format!(
        "Welcome to our newsletter!<br />\
                      Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );

    email_client
        .send_email(new_subscriber.email, "Welcome", &html_body, &plain_body)
        .await
}

///解析订阅者的表单数据
pub fn parse_subscriber(form: FormData) -> Result<NewSubscriber, String> {
    NewSubscriber::try_from(form)
}

//将插入订阅者信息的操作单独为一个函数，并为该函数“插桩”
#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(new_subscriber, pool)
)]
pub async fn insert_subscriber(
    new_subscriber: &NewSubscriber,
    pool: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at,status) VALUES ($1, $2, $3, $4,'confirmed')"#,
        Uuid::new_v4(),
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(), //仅读取信息
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

///对订阅者的名字进行验证约束，满足返回true;不满足返回，false.
pub fn is_valid_name(s: &str) -> bool {
    //检查是否为空
    let is_empty_or_whitespace = s.trim().is_empty();

    //检查名字长度是否合法,graphemes()函数返回一个，
    // is_extend 参数表示能将多个unicode码组合的识别为一个视觉字符
    let is_too_long = s.graphemes(true).count() > 256;

    //遍历输入`s`中的所有字符，检查他们是否与禁用数组中的字符匹配
    let forbidden_characters = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
    let contains_forbidden_charaters = s.chars().any(|g| forbidden_characters.contains(&g)); //只要有一个true 就会直接返回

    //如果不满足任意一个条件则返回 `false`
    !(is_empty_or_whitespace || is_too_long || contains_forbidden_charaters)
}
