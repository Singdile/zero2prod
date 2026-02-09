//！ src/routes/subscriptions_confirm.rs

use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

/// TODO: 错误处理
///web::Query<Parameters> 会反序列化请求中的查询字符串中是否包含字段subscription_token,构造Parameters,
///如果没有,直接返回 400 Bad Request
#[tracing::instrument(name = "Confirm a pending subscribe", skip(parameters, pool))]
pub async fn confirm(parameters: web::Query<Parameters>, pool: web::Data<PgPool>) -> HttpResponse {
    //根据subscription_token 查询到对应的订阅者id
    let id = match get_subscriber_id_from_token(&parameters.subscription_token, &pool).await {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    //根据订阅者id,修改对对应的订阅状态status
    match id {
        Some(subscriber_id) => {
            //id存在
            if confirm_subscriber(&pool, subscriber_id).await.is_err() {
                return HttpResponse::InternalServerError().finish(); //id,token 都正确,但是确认失败,这是服务器内部错误
            }
            return HttpResponse::Ok().finish();
        }
        None => return HttpResponse::Unauthorized().finish(), //id不存在.未授权 401
    }
}

///根据订阅者的token,查询对应在数据库中的id
#[tracing::instrument(name = "Get subscriber_id from token", skip(subscription_token, pool))]
pub async fn get_subscriber_id_from_token(
    subscription_token: &str,
    pool: &PgPool,
) -> Result<Option<Uuid>, sqlx::Error> {
    //返回的结果是匿名结构体,包含查询的字段
    let result = sqlx::query!(
        r#"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1"#,
        subscription_token
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(result.map(|r| r.subscriber_id))
}

///根据订阅者id,修改对对应的订阅状态status
#[tracing::instrument(
    name = "Mark subscriber status as confirmed",
    skip(pool, subscriber_id)
)]
pub async fn confirm_subscriber(pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status ='confirmed' WHERE id = $1"#,
        subscriber_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failedt to execute query: {:?}", e);
        e
    })?;

    Ok(())
}
