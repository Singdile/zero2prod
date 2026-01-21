//! tests/api/health_check.rs
use crate::helper::spawn_app;

#[tokio::test]
async fn health_check_work() {
    //准备
    let app = spawn_app().await;
    //引入reaWest对应用程序执行http请求
    let client = reqwest::Client::new();

    //执行
    let response = client
        .get(format!("{}/health_check", &app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    //断言
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}
