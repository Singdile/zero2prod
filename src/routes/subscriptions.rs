//! src/routes/subscriptions.rs
//
use crate::email_client::EmailClient;
use crate::startup::ApplicationBaseUrl;
use crate::{domain::NewSubscriber, email_client};
use actix_web::ResponseError;
use actix_web::{HttpResponse, web};
use anyhow::Context;
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};
use reqwest::StatusCode;
use sqlx::Transaction;
use sqlx::{PgPool, Postgres};
use std::fmt::Display;
use std::ops::Deref;
use std::sync;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

///为函数专注于业务逻辑的处理，将日志等“插桩”信息交给过程宏,值得注意的是在默认的情况下面，tracing::instrument 会将所有传递给函数的参数都放入到跨度的上下文中，必须指明日志中不需要的输入
///时刻注意这个不需要的日志信息是非常危险的，可能会导致信息泄漏,采用secrecy::Secret 来避免这个问题

///// 处理用户订阅请求的核心业务流程
///
/// 本函数实现完整的订阅工作流：验证表单 → 持久化订阅者 → 生成令牌 → 发送确认邮件。
/// 通过 `#[tracing::instrument]` 宏自动记录结构化日志（含订阅者邮箱/姓名字段），便于分布式追踪。
///
/// # 流程说明
/// 1. **表单解析**：验证并转换 `FormData` 为内部订阅者结构
/// 2. **数据库写入**：将订阅者信息存入 `subscribers` 表
/// 3. **令牌管理**：生成唯一 `subscription_token` 并存入 `subscription_tokens` 表
/// 4. **邮件触发**：发送含确认链接的验证邮件（链接基于 `base_url` 构建）
///
/// # 参数
/// * `form` - 包含用户提交的订阅表单数据（邮箱、姓名等），经 `web::Form` 封装
/// * `pool` - PostgreSQL 连接池（`web::Data<PgPool>`），用于数据库操作
/// * `email_client` - 邮件发送客户端（`web::Data<EmailClient>`），负责发送确认邮件
/// * `base_url` - 应用基础 URL（`web::Data<ApplicationBaseUrl>`），用于生成邮件中的确认链接
///
/// # 返回
/// * `200 OK` - 所有步骤成功完成（订阅者已创建 + 令牌已存储 + 邮件已发送）
/// * `400 Bad Request` - 表单数据验证失败（邮箱格式错误、必填字段缺失等）
/// * `500 Internal Server Error` - 任一后端操作失败（数据库写入、令牌存储、邮件发送）
///
/// # 错误处理特点
/// * **防御式返回**：任一环节失败立即终止流程并返回对应 HTTP 状态码
/// * **无敏感信息泄露**：错误详情仅通过 `tracing` 记录（见各子函数），响应体不包含技术细节
/// * **事务边界说明**：当前实现为**非原子操作**（插入订阅者 → 插入令牌 → 发邮件），存在中间状态风险：
///
/// # 可观测性
/// * 通过 `tracing` 自动记录：
///   - Span 名称：`"Adding a new subscriber"`
///   - 关键字段：`subscriber_email`, `subscriber_name`（脱敏后用于日志过滤）
///   - 错误详情：各子函数内部使用 `tracing::error!` 记录具体失败原因
///
/// # 示例响应
/// ```http
/// POST /subscriptions
/// Content-Type: application/x-www-form-urlencoded
///
/// name=Alice&email=alice@example.com
///
/// → 200 OK (订阅流程启动，确认邮件已发送)
/// ```
#[tracing::instrument(name = "Adding a new subscriber", skip(form,pool,email_client,base_url), fields (subscriber_email = %form.email, subscriber_name = %form.name))]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?; //将err(String) 手动转换为 SubscriberError(String)

    //使用postgres的事务,将插入订阅者信息和插入订阅者的token的操作组合为一个事件,
    //原子化操作使得数据库的结果： 1.成功插入订阅者信息和token 2.信息和token都没有插入
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool.")?;

    //插入失败，返回500服务器内部错误;插入成功,返回对应的订阅者id
    let subscriber_id = insert_subscriber(&new_subscriber, &mut transaction)
        .await
        .context("Failed to insert new subscriber in the database.")?;

    //插入订阅者的subscription_token
    let subscription_token = generate_subscription_token(); //产生订阅者的 subscription_token
    store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .context("Failed to store the confirmation token for a new subscriber.")?;

    //commit提交事务,否则默认会回滚
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new subscriber.")?;

    //插入成功，为新的订阅者发送一封确认邮件
    // if send_confirmation_email(
    //     &email_client,
    //     new_subscriber,
    //     &base_url.0,
    //     &subscription_token,
    // )
    // .await
    // .is_err()
    // {
    //     return Ok(HttpResponse::InternalServerError().finish());
    // }
    send_confirmation_email(
        &email_client,
        new_subscriber,
        &base_url.0,
        &subscription_token,
    )
    .await
    .context("Failed to send a confirmation email.")?;

    Ok(HttpResponse::Ok().finish())
}

///通过邮件服务商，向用户发送确认链接的邮件
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );
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
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    new_subscriber: &NewSubscriber,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4(); // NOTE: 订阅者的标识符,方便后面存储对应的subscription_token
    sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at,status) VALUES ($1, $2, $3, $4,'pending_confirmation')"#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(), //仅读取信息
        Utc::now()
    )
    .execute(transaction)
    .await
    .map_err(|e| {
        tracing::error!("Failed tp execute query: {:?}", e);
        e
    })?;

    Ok(subscriber_id)
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

///生成随机的长度为25个字符且大小写敏感的订阅令牌
fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric)) //distr 分布,这里表示的分布是 a-z,A-Z,0-9 (ascII)
        .map(char::from)
        .take(25)
        .collect()
}

///将订阅者的subscription_token存入数据库中
#[tracing::instrument(
    name = "Store subscription token in the database",
    skip(transaction, subscription_token, subscriber_id)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token,subscriber_id) VALUES ($1,$2)"#,
        subscription_token,
        subscriber_id
    )
    .execute(transaction)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query:{:?}", e); // NOTE: 错误级别日志宏记录
        StoreTokenError(e) // NOTE: 将底层的的查询错误转换为StoreTokenError
    })?;

    Ok(())
}

// 用于包装 `sqlx::Error` 的新类型，方便为其实现特征
pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a subscription token."
        )
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
// impl ResponseError for StoreTokenError {} //store token 应该与REST 或 HTTP 没有关系，但是我们却为其返回的错误类型实现了Web框架特征
impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

///错误传播链,直到打印出底层错误
fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

/// NOTE: 错误信息传递
/// # 信息传递路径举例
/// 1.store_token()函数内部插入token失败，返回sqlx::Error且sqlx::Error 被包装成为 StoreTokenError(sqlx::Error) 向上返回
/// 2.subscribe()函数中，将返回的StoreTokenError(sqlx::Error)转换为SubscribeError::StoreTokenError(StoreTokenError(sqlx::Error))
/// 3.SubscriberError向上返回，转换为actix-web::Error
/// NOTE: 4.actix-web框架中的中间件tracing_actix_web::TracingLogger 会捕获这个错误，在集成的日志系统中记录信息，通常是调用Debug实现，然后便会调用到内部的SubscriberError的Debug实现
///
///使用SubscribeError，将订阅发生的错误与Http之间的逻辑关系链接起来，而不是直接将底层错误比如存储令牌失败与Http之间关联.
///因为有些时候，单独调用插入token，不希望返回的Error的是Web的Error
#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")] //error("") 用于实现SubscribeError 关于ValidationError变体的display
    ValidationError(String), //调用者应知道的错误

    //transparent直接包装实现SubscriberError 关于UnexpectedError变体的display,source， 即直接调用底层的display 和 source
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error), //调用者不应该直接知道的底层错误
                                            // DatabaseError(sqlx::Error), DatabseError代表了多种情况，在display的时候，没有办法分清楚场景，需要为每一个场景添加变体 #[error("Failed to acquire a Postgres connection from the pool.")]
                                            // PoolError(#[source] sqlx::Error), //获取连接池错误 source为SubscriberError 实现std::error::Error,用于获取底层错误信息

                                            // #[error("Failed to insert new subscriber in the database.")]
                                            // InsertSubsciberError(#[source] sqlx::Error), //插入订阅者错误

                                            // #[error("Failedt to store the confirmation token for a new subscriber.")]
                                            // StoreTokenError(#[from] StoreTokenError), //存储订阅者的token错误 from 实现错误类型向上转换`

                                            // #[error("Failedt to commit SQL transaction to store a new subscriber.")]
                                            // TransactionError(#[source] sqlx::Error), //提交数据库事件错误

                                            // #[error("Failed to send a confirmation email.")]
                                            // SendEmailError(#[from] reqwest::Error),
}

///实现Debug，方便在actix-web中表现
impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(&self, f)
    }
}

/// NOTE: 查看 `actix_web::Error` 的实现，发现它为所有实现了 `ResponseError` 的类型
/// 自动实现了 `From`：
///
/// ```rust
/// impl<T: ResponseError + 'std> From<T> for Error {
///     fn from(error: T) -> Self {
///         Error::from_response_error(error)
///     }
/// }
/// ```
///
/// 这意味着：**任何实现了 `ResponseError` 的错误类型（如 `SubscribeError`）都可以直接用 `?`
/// 转换为 `actix_web::Error`**。
///
/// `ResponseError` 特征要求实现：
/// ```rust
/// fn status_code(&self) -> StatusCode;
/// fn error_response(&self) -> HttpResponse; // 注意：返回 HttpResponse，不是 BoxBody
/// ```
///
/// NOTE: 其**默认实现**会使用 `self` 的 `Display` 输出作为响应体（通过 `to_string()`），
/// 但我们可以重写 `error_response` 来返回自定义 JSON 或其他格式。
impl ResponseError for SubscribeError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
} //ResponseError 特征默认实现的方法是返回A 500 Internal Server Error的HttpResponse
