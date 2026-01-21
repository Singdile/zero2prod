//! tests/api/helper.rs
use fake::faker::address;
use once_cell::sync::Lazy;
use secrecy::Secret;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;
use zero2prod::configuration::{DatabaseSettings, get_configuration};
use zero2prod::email_client::EmailClient;
use zero2prod::startup::{build,run,get_connection_pool};
use zero2prod::telemetry::{get_sunscriber, init_subscriber};
use zero2prod::startup::Application;

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

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


//在后台启动应用程序,将服务程序绑定的addr返回(http://127.0.0.1:XXXX)
pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);
    

    //读取配置文件信息
    let  configuration = {
	let mut c = get_configuration().expect("Failed to readn configuration.");
	//每次测试使用不同的数据库名字
	c.database.database_name = Uuid::new_v4().to_string();
	//使用随机端口
	c.application.port = 0;
	c
    };

    //创建和迁移数据库
    configure_database(&configuration.database).await;

    //构建applicaion
    let application = Application::build(configuration.clone()).await.expect("Failed to build application.");
    
    //返回服务器访问端口号
    let address = format!("http://localhost:{}", application.port());

    
    //spawn创建一个tokio task,将server放在上面去执行，立即返回执行下面的代码
    // 通常下面的代码是同task 是没有什么关系的，但是如果有要用到task的返回结果，那么就会在需要的位置执行.await()
    let _ = tokio::spawn(application.run_until_stopped());

    TestApp {
        address,
        db_pool: get_connection_pool(&configuration.database),
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
