use sqlx::PgPool;
use std::net::TcpListener;
use tracing::Subscriber;
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt}; //Registry 实现了Subscriber特征
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_sunscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    //设置全局的订阅器
    let subscriber = get_sunscriber("zero2prod".into(), "info".into());
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
