use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::email_client::EmailClient;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_sunscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    //设置全局的订阅器
    let subscriber = get_sunscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    //读取配置
    let configuration = get_configuration().expect("Failed to read configuraion.");
    //连接数据库
    let connection_pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.database.with_db());

    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    let listener = TcpListener::bind(address)?;

    //使用configuration 构建一个 EmailClient
    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email address");
    let email_client = EmailClient::new(
        configuration.email_client.base_url,
        sender_email,
        configuration.email_client.authorization_token,
    );

    //为run添加一个新参数 email_client
    run(listener, connection_pool, email_client)?.await?;
    Ok(())
}
