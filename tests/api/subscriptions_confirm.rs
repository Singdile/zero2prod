//! tests/api/subscriptions_confirm.rs

use sqlx::query;
use wiremock::{Mock, ResponseTemplate, matchers::method};

use crate::helper::spawn_app;
use reqwest::Url;
use wiremock::matchers::path;

///用户提交订阅邮件之后,会生成一个token,关联到订阅者的ID
///将token拼接到链接中,通过Postmark发送给用户
///用户点击链接,我们的服务器收到带有Token的Get请求
///1.在数据库中查询是否有这个token
///2.如果存在并且没有过期，将订阅者的状态改为confirmed

///测试: 没有token的订阅确认会被拒绝
#[tokio::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    //准备
    let app = spawn_app().await;

    //执行
    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    //断言
    assert_eq!(400, response.status().as_u16());
}

///发送给用户的确认邮件里面的HtmlBody 和 TextBody 里面的链接应该一样
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

    app.post_subscriptions(body.into()).await; //发送订阅邮件

    //执行
    //点击确认链接
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_link = app.get_confirmation_link(&email_request);

    //断言
    assert_eq!(confirmation_link.html, confirmation_link.plain_text);
}

///点击确认链接,应该会收到200 OK 的回复
#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    //准备
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await; //发送订阅邮件

    //执行
    //点击确认链接
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_link = app.get_confirmation_link(&email_request);
    let response = reqwest::get(confirmation_link.html).await.unwrap();

    //断言
    assert_eq!(response.status().as_u16(), 200);
}

///检查订阅者的status记录
#[tokio::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscription() {
    //准备
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await; //发送订阅邮件

    //获取确认链接,并点击
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_link = app.get_confirmation_link(&email_request);
    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    //断言
    let saved = query!("SELECT email,name,status FROM subscriptions") //query! 返回一个匿名结构体,包含查询的字段
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "confirmed");
}
