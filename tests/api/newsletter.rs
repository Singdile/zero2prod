//! tests/api/newsletter.rs

use crate::helper::{ConfirmationLinks, TestApp, spawn_app};
use wiremock::matchers::{any, method};
use wiremock::{Mock, ResponseTemplate, matchers::path};

/// 发送新的邮件信息，不应该包括没有确认的订阅用户
#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfimed_subscribers() {
    //准备
    let app = spawn_app().await;
    create_uncomfirmed_subscriber(&app).await; //创建一个未确认的订阅者

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        //断言Postmark没有发送任何请求
        .expect(0)
        .mount(&app.email_server)
        .await;

    //执行

    //邮件简报负责的骨架
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML<p>",
        }
    });

    let response = app.post_newsletters(newsletter_request_body).await;

    //断言
    assert_eq!(response.status().as_u16(), 200); //确保调用接口 /newsletter 成功
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    //准备
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    //执行

    //邮件简报负责的骨架
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML<p>",
        }
    });

    let response = app.post_newsletters(newsletter_request_body).await;

    //断言
    assert_eq!(response.status().as_u16(), 200); //确保调用接口 /newsletter 成功
}

///验证发送到端点 /newsletter 的request数据是否正确
#[tokio::test]
async fn newsletter_return_400_for_invalid_data() {
    //准备
    let app = spawn_app().await;
    let test_case = vec![
        (
            serde_json::json!({
                "content": {
                    "text": "Newsletter body as plain text",
                    "html": "<p>Newsletter body as HTML</p>"
                }
            }),
            "missing title",
        ),
        (
            serde_json::json!({
                         "title":"Newsletter"}),
            "missing content",
        ),
    ];
    //执行
    for (invalidbody, error_message) in test_case {
        let response = app.post_newsletters(invalidbody).await;

        //断言
        assert_eq!(
            response.status().as_u16(),
            400,
            "The API did not fail with 400 Bad Requset when the payload was {}",
            error_message
        );
    }

    //断言
}

///创建一个确认的订阅用户
async fn create_confirmed_subscriber(app: &TestApp) {
    let comfirmation_link = create_uncomfirmed_subscriber(app).await;
    //点击确认链接
    reqwest::get(comfirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

///创建一个没有确认的订阅用户，并返回对应的确认链接
async fn create_uncomfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    //因为发送订阅邮件之后，会通过postmark发送一封确认邮件信息，所以这里需要模拟服务器收到邮件信息
    //否则，app.post_subscripitons就会失败
    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create uncofirmed subsciber")
        .expect(1)
        .mount_as_scoped(&app.email_server) //scoped mock，作用域模拟，只在该作用域有效，变量_mock_guard drop的同时，也会drop该规则并检查mock的期望是否得到满足
        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    //检查mock的Postmark的服务器收到的请求，获取确认链接并将其返回
    let email_request = app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_link(&email_request)
}
