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
        match subscriber {
            Ok(confirmedsubsciber) => email_client
                .send_email(
                    &confirmedsubsciber.email,
                    &body.title,
                    &body.content.html,
                    &body.content.text,
                )
                .await
                .with_context(|| {
                    format!(
                        "Failed to send newsletter issue to {}",
                        confirmedsubsciber.email
                    )
                })?,
            Err(error) => {
                //这里的？是tracing库的特殊用法，和rust的语法中的？没有关系。 会记录变量的debug信息到日志中去
                tracing::warn!(error.cause_chain = ?error, "Skipping a confirmed subscriber.\
                                                            Their stored contact details are invalid");
            }
        }
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
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers =
        sqlx::query!(r#"SELECT email FROM subscriptions WHERE status = 'confirmed'"#)
            .fetch_all(pool)
            .await?
            //查询返回的是匿名结构体，包含字段email
            .into_iter()
            .map(|r| match SubscriberEmail::parse(r.email) {
                Ok(email) => Ok(ConfirmedSubscriber { email }),
                Err(error) => Err(anyhow::anyhow!(error)),
            })
            .collect(); //anyhow::Error  为所有具有std::error::Error特征的错误，实现from.从而，使得sqlx::Error 能够通过from转换为anyhow::Error

    Ok(confirmed_subscribers)
}
