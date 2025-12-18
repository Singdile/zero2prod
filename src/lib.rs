use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web::dev::Server;
use std::net::TcpListener;
///异步函数，处理网络请求通常需要等待I/O,使用异步让出CPU资源
async fn health_check(_req: HttpRequest) -> impl Responder {
    HttpResponse::Ok().finish()//HttpResponse::OK() 获取一个HttpResponseBuilder,然后生成一个HttpResponse
}


///服务器启动函数,接受address作为服务器绑定的socket地址
pub fn run(address: TcpListener) -> Result<Server, std::io::Error> {
    //HttpServer::new()创建一个HTTP服务器实例bind()绑定IP:端口
    // run()将绑定成功的Httpserver 转换为一个运行中(监听端口)的server实例,类型:
    //use actix_web::dev::Server;
    let server = HttpServer::new(|| {
       App::new()
        .route("/health_check", web::get().to(health_check))
    })
    .listen(address)?
    .run();

    Ok(server)

}
