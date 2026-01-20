use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::email_client::EmailClient;
use zero2prod::startup::{build, run};
use zero2prod::telemetry::{get_sunscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    //设置全局的订阅器
    let subscriber = get_sunscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    //读取配置
    let configuration = get_configuration().expect("Failed to read configuraion.");
    
    //启动后端服务
    let server = build(configuration).await?;
    
    server.await?;

    Ok(())
}
