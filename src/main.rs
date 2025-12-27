use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_sunscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    //设置全局的订阅器
    let subscriber = get_sunscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    //读取数据库配置
    let configuration = get_configuration().expect("Failed to read configuraion.");
    //连接数据库
    let connection_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to Postgres");
    let address = format!("127.0.0.1:{}", configuration.application_port);
    let listener = TcpListener::bind(address)?;

    run(listener, connection_pool)?.await
}
