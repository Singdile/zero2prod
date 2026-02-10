//! src/routes/newsletters.rs
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::error_chain_fmt;
use actix_web::HttpResponse;
use actix_web::ResponseError;
use actix_web::http::StatusCode;
use actix_web::web;
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;

///保存发送的新邮件的数据结构
#[derive(Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(Deserialize)]
pub struct Content {
    text: String,
    html: String,
}

///发送新的邮件的端口的错误类型
#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

///实现Debug，逐层打印错误链接
impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

///为PublishError实现ResponseError特征，方便PublishError转换为actix_web::Error
impl ResponseError for PublishError {
    fn status_code(&self) -> StatusCode {
        match self {
            PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

///发送新的邮件到已经订阅的用户
///1. 检查调用该端点的信息是否符合条件，即发送邮件的信息是否合法。
///2. 查询数据库中确认之后的用户信息
///3. 发送新的邮件信息
///
///注意：actix_web::web::Json<BodyData>，解析信息的时候无法填充完BodyData的字段，actix_web就会生成一个400 Bad Request 响应直接返回
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, PublishError> {
    let subscribers = get_confirmed_subscribers(&pool).await?;
    for subscriber in subscribers {
        email_client
            .send_email(
                subscriber.email,
                &body.title,
                &body.content.html,
                &body.content.text,
            )
            .await
            //anyhow::context 为Result实现了 Context 方法，Context方法，将Result<T,E> 转换为 Result<T,anyhow::Error>，并携带更多的信息
            .with_context(|| format!("Failed to send newsletter issue to {}", subscriber.email));
    }

    Ok(HttpResponse::Ok().finish())
}

///确认订阅的订阅者
struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

///获取已订阅的订阅者列表
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<ConfirmedSubscriber>, anyhow::Error> {
    //内部定义Row，来便捷地直接通过sqlx::query_as!获取查询的数据
    struct Row {
        email: String,
    }

    let rows = sqlx::query_as!(
        Row,
        r#"SELECT email FROM subscriptions WHERE status = 'confirmed'"#
    )
    .fetch_all(pool)
    .await?; //anyhow::Error  为所有具有std::error::Error特征的错误，实现from.从而，使得sqlx::Error 能够通过from转换为anyhow::Error

    //将获取的数据转换为符合条件的数据格式
    let confirmed_subscribers = rows
        .into_iter()
        .filter_map(|r| match SubscriberEmail::parse(r.email) {
            //filter_map 返回迭代器，保留闭包中Some(value) 的value值，忽略None
            Ok(email) => Some(ConfirmedSubscriber { email }),
            Err(err) => {
                tracing::warn!(
                    "A confimed subscirber is using an invalid email address.\n {}",
                    err
                ); //日志记录有效订阅者的无效地址；无效地址的出现有很多可能的原因，比如修改了邮件验证逻辑，导致之前的邮件地址确实是有效的，但是现在不再有效了
                None
            }
        })
        .collect();

    Ok(confirmed_subscribers)
}
