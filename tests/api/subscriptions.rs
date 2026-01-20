//! test/api/subscriptions.rs
use crate::helper::spawn_app;

#[tokio::test]
///测试合法数据是否能订阅成功
async fn subscribe_returns_a_200_for_valid_form_data() {
    //准备
    let app = spawn_app().await; //需要这里的返回值，所以调用await，执行并等待返回
    let client = reqwest::Client::new();

    //执行
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(format!("{}/subscriptions", &app.address))
        .header("Content-type", "application/x-www-form-urlencoded") //http头部信息，表示传输的是表单信息
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    //断言
    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

#[tokio::test]
///超文本传输协议（HTTP）400 Bad Request 响应状态码表示服务器因某些被认为是客户端错误的原因（例如，请求语法错误、无效请求消息格式或者欺骗性请求路由），而无法或不会处理该请求。
async fn subscribe_returns_a_400_when_data_is_missing() {
    //准备
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let test_case = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmailc.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    //执行
    for (invalid_body, error_message) in test_case {
        let response = client
            .post(format!("{}/subscriptions", &app.address))
            .header("Content-type", "application/x-www-form-urlencoded") //http头部信息，表示传输的是表单信
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");
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
    let client = reqwest::Client::new();

    let test_case = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    //执行
    for (body, description) in test_case {
        //执行
        let response = client
            .post(format!("{}/subscriptions", app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.");

        //断言判断
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 200 OK when the payload was {}", //payload 真正关心的数据，比如这里说的是body部分
            description
        );
    }
}
