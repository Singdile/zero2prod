// !tests/health_check.rs
use zero2prod;
use std::net::TcpListener;
#[tokio::test]
async fn health_check_work() {

    //准备
    let address = spawn_app();
    //引入reaWest对应用程序执行http请求
    let client = reqwest::Client::new();

    //执行
    let response = client
        .get(&format!("{}/health_check",&address))
        .send()
        .await
        .expect("Failed to execute request.");

   //断言
    assert!(response.status().is_success());
   assert_eq!(Some(0),response.content_length());
}


//在后台启动某处的应用程序,将服务程序绑定的addr返回
fn spawn_app() -> String{
    //首先获得系统绑定的socket地址
    let listener = TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind random port");

    //检查系统分配的端口号
    let port = listener.local_addr().unwrap().port();
    let server = zero2prod::run(listener).expect("Failed to bind address");

    //spawn创建一个tokio task,将server放在上面去执行，立即返回执行下面的代码
    // 通常下面的代码是同task 是没有什么关系的，但是如果有要用到task的返回结果，那么就会在需要的位置执行.await()
    let _ = tokio::spawn(server);


    //返回地址给调用者
    format!("http://127.0.0.1:{}", port)
}
