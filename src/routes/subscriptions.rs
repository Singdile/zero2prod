//! src/routes/subscriptions.rs
//!
use std::ops::Deref;
use std::sync;

use crate::email_client::EmailClient;
use crate::startup::ApplicationBaseUrl;
use crate::{domain::NewSubscriber, email_client};
use actix_web::{HttpResponse, web};
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};
use sqlx::PgPool;
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
    base_url: web::Data<ApplicationBaseUrl>, //TODO: 正确使用base_url
) -> HttpResponse {
    let new_subscriber = match parse_subscriber(form.0) {
        Ok(subscriber) => subscriber,
        Err(_) => return HttpResponse::BadRequest().finish(), //表单数据错误，返回(400, BAD_REQUEST, "Bad Request");
    };

    //插入失败，返回500服务器内部错误;插入成功,返回对应的订阅者id
    let subscriber_id = match insert_subscriber(&new_subscriber, &pool).await {
        Ok(subscribe_id) => subscribe_id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    //插入订阅者的subscription_token
    let subscription_token = generate_subscription_token(); //产生订阅者的 subscription_token
    if store_token(&pool, subscriber_id, &subscription_token)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish(); //存储subscription_token 失败,返回 500 服务器内部错误
    }
    //插入成功，为新的订阅者发送一封确认邮件
    if send_confirmation_email(
        &email_client,
        new_subscriber,
        &base_url.0,
        &subscription_token,
    )
    .await
    .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
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
    skip(new_subscriber, pool)
)]
pub async fn insert_subscriber(
    new_subscriber: &NewSubscriber,
    pool: &PgPool,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4(); // NOTE: 订阅者的标识符,方便后面存储对应的subscription_token
    sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at,status) VALUES ($1, $2, $3, $4,'pending_confirmation')"#,
        subscriber_id,
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
    skip(pool, subscription_token)
)]
pub async fn store_token(
    pool: &PgPool,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token,subscription_id) VALUES ($1,$2)"#,
        subscription_token,
        subscriber_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query:{:?}", e); // NOTE: 错误级别日志宏记录
        e
    })?;

    Ok(())
}
