//! tests/api/helper.rs
use fake::faker::address;
use once_cell::sync::Lazy;
use secrecy::Secret;
use sha3::Digest;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::configuration::{DatabaseSettings, get_configuration};
use zero2prod::email_client::EmailClient;
use zero2prod::startup::Application;
use zero2prod::startup::{get_connection_pool, run};
use zero2prod::telemetry::{get_subscriber, init_subscriber};

///测试服务器，包含服务器端口地址、数据库连接池
pub struct TestApp {
    pub address: String,          //应用程序地址
    pub db_pool: PgPool,          //数据库连接池
    pub email_server: MockServer, //模拟服务器，替代Postmark的API
    pub port: u16,                //服务器端口地址
    pub test_user: TestUser,      //测试用户
}

///辅助结构体，用于记录管理用户users
pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(), //uuid 转成字符串之后的格式是：36个字符的固定格式: 8-4-4-4-12 分段的十六进制字符串
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    //存储信息
    async fn store(&self, pool: &PgPool) {
        let password_hash = sha3::Sha3_256::digest(&self.password.as_bytes());
        let password_hash = format!("{:x}", password_hash);

        sqlx::query!(
            "INSERT INTO users (user_id,username, password_hash) VALUES ($1,$2,$3);",
            self.user_id,
            self.username,
            password_hash
        )
        .execute(pool)
        .await
        .expect("Failed to store test user.");
    }
}

///发送给邮件API的请求中所包含的确认链接
pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

impl TestApp {
    ///用户发送订阅信息
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/subscriptions", &self.address))
            .header("Content-type", "application/x-www-form-urlencoded") //http头部信息，表示传输的是表单信
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    ///向用户发送新的邮件
    pub async fn post_newsletters(&self, body: serde_json::Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/newsletters", &self.address))
            //增加身份验证，透过测试requests_missing_authorization_are_rejected
            .basic_auth(&self.test_user.username, Some(&self.test_user.password)) //uuid,在现实中，你需要每秒生成 10 亿个 UUID 持续 85 年，才有 50% 的概率遇到一次重复。
            .json(&body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    ///从发送给邮件API的请求中提取出确认链接
    pub fn get_confirmation_link(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap(); //确认邮件信息

        //从指定的字段提取链接
        let get_link = |s: &str| {
            //构建闭包
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();

            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            //确保调用的API是本地的
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html = get_link(&body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(&body["TextBody"].as_str().unwrap());

        ConfirmationLinks { html, plain_text }
    }
}

//声明一个静态变量，Lazy<()> 表示这是一个“懒加载”包装器，允许将这段初始化逻辑，推迟到第一次使用这个变量的时候。一旦使用，当再次调用，也只会返回第一次执行的结果。
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filiter_level = "info".into();
    let subscriber_name = "test".into();

    //如果设置了TEST_LOG 则使用std::io::stdout,否则 使用 std::io::sink
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filiter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filiter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

//在后台启动应用程序,将服务程序绑定的addr返回(http://127.0.0.1:XXXX)
pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    //启动一个模拟服务器，替代postmark的api
    let email_server = MockServer::start().await;

    //读取配置文件信息
    let configuration = {
        let mut c = get_configuration().expect("Failed to readn configuration.");
        //每次测试使用不同的数据库名字
        c.database.database_name = Uuid::new_v4().to_string();
        //使用随机端口
        c.application.port = 0;
        //使用模拟服务器作为邮件的Postmark API
        c.email_client.base_url = email_server.uri();
        c
    };

    //创建和迁移数据库
    configure_database(&configuration.database).await;

    //构建applicaion
    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application.");

    //返回服务器访问端口号
    let address = format!("http://localhost:{}", application.port());
    let application_port = application.port();

    //spawn创建一个tokio task,将server放在上面去执行，立即返回执行下面的代码
    // 通常下面的代码是同task 是没有什么关系的，但是如果有要用到task的返回结果，那么就会在需要的位置执行.await()
    let _ = tokio::spawn(application.run_until_stopped());

    let testapp = TestApp {
        address,
        db_pool: get_connection_pool(&configuration.database),
        email_server,
        port: application_port,
        test_user: TestUser::generate(),
    };

    //添加user用户
    testapp.test_user.store(&testapp.db_pool).await;

    testapp
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
