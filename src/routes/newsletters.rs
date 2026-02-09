//! src/routes/newsletters.rs
use actix_web::HttpResponse;
use actix_web::web;
use serde::Deserialize;

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

///发送新的邮件到已经订阅的用户
///1. 检查调用该端点的信息是否符合条件，即发送邮件的信息是否合法。
///2. 查询数据库中确认之后的用户信息
///3. 发送新的邮件信息
///
///注意：actix_web::web::Json<BodyData>，解析信息的时候无法填充完BodyData的字段，actix_web就会生成一个400 Bad Request 响应直接返回
pub async fn publish_newsletter(_body: web::Json<BodyData>) -> HttpResponse {
    HttpResponse::Ok().finish()
}
