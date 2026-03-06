//! src/routes/newsletters.rs
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::error_chain_fmt;
use actix_web::HttpRequest;
use actix_web::HttpResponse;
use actix_web::ResponseError;
use actix_web::http::StatusCode;
use actix_web::http::header::HeaderMap;
use actix_web::web;
use anyhow::Context;
use reqwest::header;
use reqwest::header::HeaderValue;
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Deserialize;
use sqlx::PgPool;
use sqlx::Pool;
use uuid::Uuid;

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
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),

    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error), //使用thiserror 自动实现了anyhow::Error 通过from转换为 PublishError::UnexpectedError
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
            PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR, //500,服务器内部错误
            //身份验证失败，返回401
            PublishError::AuthError(_) => StatusCode::UNAUTHORIZED,
        }
    }

    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            //内部错误，如连接服务器失败，属于服务端错误，只需返回500 服务器内部错误
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            //身份验证失败，返回401代码，并添加头部信息 WWW-AUTHENICATE
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            }
        }
    }
}

///发送新的邮件到已经订阅的用户
///1. 检查调用该端点的信息是否符合条件，即发送邮件的信息是否合法。
///2. 查询数据库中确认之后的用户信息
///3. 发送新的邮件信息
///
///注意：actix_web::web::Json<BodyData>，解析信息的时候无法填充完BodyData的字段，actix_web就会生成一个400 Bad Request 响应直接返回
#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(body, pool, email_client, request),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty) //先申明要追踪的字段，在函数内部计算后再显示赋值
)]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?; //验证失败，将error手动转换为PublishError::AuthError
    //current返回当前活跃的span的handle,然后对其进行操作
    tracing::Span::current().record("username", tracing::field::display(&credentials.username)); //使用tracing::field::display 将值包裹为Value类型，并指定用Display trait格式的内容
    let user_id = validate_credentials(credentials, &pool).await?;
    tracing::Span::current().record("user_id", tracing::field::display(&user_id));
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

///basic auth 需要的用户名和密码
struct Credentials {
    username: String,
    password: Secret<String>,
}

///提取base64编码的用户名和密码
fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    //获取头部中关于HeaderMap 的值
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string.")?;

    //字符串切割出 用户名:密码的 base64 编码
    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'")?;

    //将内容转换为字节
    let decoded_bytes = base64::decode_config(base64encoded_segment, base64::STANDARD)
        .context("Failed to base64-decode 'Basic' credentials")?;

    //解析为utf8
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF8.")?;

    //使用冒号分隔
    let mut credentials = decoded_credentials.splitn(2, ':'); //返回迭代器
    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A username must be provided in 'Basic' auth."))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A password muse be provided in 'Basic' auth."))?
        .to_string();

    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
}

/// 对调用newsletter端点的用户信息进行数据库users表的查询验证
async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    let user_id = sqlx::query!(
        r#"SELECT user_id FROM users WHERE username = $1 AND password = $2"#,
        credentials.username,
        credentials.password.expose_secret()
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to validate auth credentials.")?; //包裹原始的错误并添加提示信息，统一错误类型为anyhow::Error

    user_id
        .map(|row| row.user_id)
        .ok_or_else(|| anyhow::anyhow!("Invaid username or password."))
        .map_err(PublishError::AuthError)
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
