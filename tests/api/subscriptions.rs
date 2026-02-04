//! test/api/subscriptions.rs
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::helper::spawn_app;

/// 测试订阅功能：发送有效的表单数据应返回 200 OK
/// 此测试验证：
/// - API 端点接受有效的 name 和 email
/// - 响应状态码为 200
#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    //准备
    let app = spawn_app().await; //需要这里的返回值，所以调用await，执行并等待返回
    //执行
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    //模拟邮件服务器的反应
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    //执行
    let response = app.post_subscriptions(body.to_string()).await;

    //断言
    assert_eq!(200, response.status().as_u16());
}

///测试订阅者数据是否正常存入数据库,持久化了
#[tokio::test]
async fn subscribe_persists_the_new_subscriber() {
    //准备
    let app = spawn_app().await; //需要这里的返回值，所以调用await，执行并等待返回
    //执行
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    //模拟邮件服务器的反应
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    //执行
    let _response = app.post_subscriptions(body.to_string()).await;

    //断言
    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "pending_confirmation");
}

#[tokio::test]
///超文本传输协议（HTTP）400 Bad Request 响应状态码表示服务器因某些被认为是客户端错误的原因（例如，请求语法错误、无效请求消息格式或者欺骗性请求路由），而无法或不会处理该请求。
async fn subscribe_returns_a_400_when_data_is_missing() {
    //准备
    let app = spawn_app().await;
    let test_case = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmailc.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    //执行
    for (invalid_body, error_message) in test_case {
        let response = app.post_subscriptions(invalid_body.to_string()).await;
        //断言
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the playload was {}",
            error_message
        );
    }
}

///用一些有问题的输入来测试API
#[tokio::test]
async fn subscribe_returns_a_400_when_fields_are_present_but_invalid() {
    //准备
    let app = spawn_app().await;
    let test_case = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    //执行
    for (body, description) in test_case {
        //执行
        let response = app.post_subscriptions(body.to_string()).await;
        //断言判断
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 200 OK when the payload was {}", //payload 真正关心的数据，比如这里说的是body部分
            description
        );
    }
}

///用户订阅之后，需要向用户发送确认订阅的邮件信息
#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    //准备
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    //执行
    app.post_subscriptions(body.to_string()).await;

    //断言判断
    //Mock 会在析构的时候检查断言
}

///检测发送到模拟的邮件服务商的邮件时候包含链接
#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
    //准备
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    //执行
    app.post_subscriptions(body.to_string()).await;

    //断言
    let email_request = &app.email_server.received_requests().await.unwrap()[0]; //返回MockServer接收到的所有请求,以Vec的形式,获取一个请求
    let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap(); //将正文部分从二进制转换为JSON格式

    //从指定的字段提取链接
    let get_link = |s: &str| {
        //构建闭包
        let links: Vec<_> = linkify::LinkFinder::new()
            .links(s)
            .filter(|l| *l.kind() == linkify::LinkKind::Url)
            .collect();

        assert_eq!(links.len(), 1);

        links[0].as_str().to_owned()
    };

    let html_link = get_link(&body["HtmlBody"].as_str().unwrap());
    let text_link = get_link(&body["TextBody"].as_str().unwrap());

    //这两个链接应该是一样的
    assert_eq!(html_link, text_link);
}

///错误处理,当数据库操作发生错误,应当返回详细的错误报告给操作人员和简略的错误报告给用户
#[tokio::test]
async fn subscribe_fails_if_there_is_a_fatal_database_error() {
    //准备
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    //删除subscription_tokens 表的列 subscription_token,导致订阅插入用户token到数据库失败
    // sqlx::query!(r#"ALTER TABLE subscription_tokens DROP COLUMN subscription_token;"#,)
    //     .execute(&app.db_pool)
    //     .await
    //     .unwrap();

    sqlx::query!(r#"ALTER TABLE subscriptions DROP COLUMN email;"#,)
        .execute(&app.db_pool)
        .await
        .unwrap();

    //执行
    let response = app.post_subscriptions(body.to_string()).await;

    //断言
    assert_eq!(response.status().as_u16(), 500);
}
