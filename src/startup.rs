//! src/startup.rs

use crate::configuration;
use crate::configuration::DatabaseSettings;
use crate::configuration::{Settings, get_configuration};
use crate::email_client::EmailClient;
use crate::routes::{confirm, publish_newsletter};
use crate::routes::{health_check, subscribe};
use actix_web::dev::Server;
use actix_web::{App, HttpServer, web};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger; //使用TracingLogger 为每一次请求分配一个唯一的ID,创建一个新的span //客户端

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    //将 build 函数作为 Applicationd 的构造函数
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        //连接数据库
        let connection_pool = get_connection_pool(&configuration.database);

        //使用configuration 构建一个 EmailClient
        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender email address");

        let time_out = configuration.email_client.timeout();
        let email_client = EmailClient::new(
            configuration.email_client.base_url,
            sender_email,
            configuration.email_client.authorization_token,
            time_out,
        );

        //主机地址和对应的程序端口
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(address)?;

        //因为这里是随机分配的端口，所以，要提取出来
        let port = listener.local_addr().unwrap().port();

        //为run添加一个新参数 email_client
        let server = run(
            listener,
            connection_pool,
            email_client,
            configuration.application.base_url,
        )?;

        Ok(Self { port, server })
    }

    ///返回服务器访问端口号
    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

///获取连接池
pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.with_db())
}

///定义一个包装类型
///在actix-web中,上下文获取数据的方式是基于类型的
///所以,使用包装类,避免String类型重复冲突
pub struct ApplicationBaseUrl(pub String);

pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    base_url: String,
) -> Result<Server, std::io::Error> {
    //为什么要传闭包？ 因为 Actix-web 需要为每一个 Worker 线程都创建一个独立的 App 实例。
    //多实例并行：如果你有 8 个线程，Actix 就会运行这个闭包 8 次，产生 8 个相互隔离的 App 对象
    //所以，为了给每个App副本提供一个连接，需要连接是可以克隆的，但是PgConnection是没有实现clone的，这是一个系统资源——与Postgres的Tcp连接
    //
    //在actix-web中,上下文获取数据的方式是基于类型的
    //
    let db_pool = web::Data::new(db_pool); //将连接包裹在一个智能指针
    let email_client = web::Data::new(email_client); //将客户端实例包裹在一个智能指针，方便复用
    let base_url = web::Data::new(ApplicationBaseUrl(base_url)); //暴露给用户发送邮件的链接地址
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(publish_newsletter))
            .app_data(db_pool.clone()) //获取一个智能指针的副本，并将其绑定
            .app_data(email_client.clone()) //获取一个智能指针的副本，并将其绑定
            .app_data(base_url.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
