// !tests/health_check.rs
use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;
use zero2prod::configuration::{DatabaseSettings, get_configuration};
use zero2prod::startup::run;
use zero2prod::telemetry::{get_sunscriber, init_subscriber};

//声明一个静态变量，Lazy<()> 表示这是一个“懒加载”包装器，允许将这段初始化逻辑，推迟到第一次使用这个变量的时候。一旦使用，当再次调用，也只会返回第一次执行的结果。
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filiter_level = "info".into();
    let subscriber_name = "test".into();

    //如果设置了TEST_LOG 则使用std::io::stdout,否则 使用 std::io::sink
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_sunscriber(subscriber_name, default_filiter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_sunscriber(subscriber_name, default_filiter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

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

//在后台启动应用程序,将服务程序绑定的addr返回(http://127.0.0.1:XXXX)
async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    //首先获得系统绑定的socket地址
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");

    //检查系统分配的端口号
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    //读取配置文件中的数据库连接信息
    let mut configuration = get_configuration().expect("Failed to read configuration");
    configuration.database.database_name = Uuid::new_v4().to_string();
    let connection_pool = configure_database(&configuration.database).await;

    let server = run(listener, connection_pool.clone()).expect("Failed to bind address");

    //spawn创建一个tokio task,将server放在上面去执行，立即返回执行下面的代码
    // 通常下面的代码是同task 是没有什么关系的，但是如果有要用到task的返回结果，那么就会在需要的位置执行.await()
    tokio::spawn(server);

    TestApp {
        address,
        db_pool: connection_pool,
    }
}

///连接上postgres系统数据库，创建一个新的数据库，然后建立与新数据库的连接池PgPool,并返回
pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
    //创建数据库
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to Postgres");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database");

    //迁移数据库
    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to Postgres.");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}

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
            .header("Content-type", "application/x-www-form-urlencoded") //http头部信息，表示传输的是表单信息
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
