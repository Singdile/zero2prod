//! src/startup.rs

use crate::routes::{health_check, subscribe};
use actix_web::dev::Server;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use sqlx::PgPool;
use std::net::TcpListener;

pub fn run(listener: TcpListener, db_pool: PgPool) -> Result<Server, std::io::Error> {
    //为什么要传闭包？ 因为 Actix-web 需要为每一个 Worker 线程都创建一个独立的 App 实例。
    //多实例并行：如果你有 8 个线程，Actix 就会运行这个闭包 8 次，产生 8 个相互隔离的 App 对象
    //所以，为了给每个App副本提供一个连接，需要连接是可以克隆的，但是PgConnection是没有实现clone的，这是一个系统资源——与Postgres的Tcp连接
    //
    //
    //
    let db_pool = web::Data::new(db_pool); //将连接包裹在一个智能指针
    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .app_data(db_pool.clone()) //获取一个智能指针的副本，并将其绑定
    })
    .listen(listener)?
    .run();

    Ok(server)
}
