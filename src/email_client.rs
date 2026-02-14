//! src/email_client.rs

use std::time::Duration;

use crate::domain::SubscriberEmail;
use config::builder;
use reqwest::{Client, ClientBuilder, Response};
use secrecy::{ExposeSecret, Secret};
use serde;
///邮件客户端,将状态存储到数据结构,将行为放在impl实现
///通过邮件客户端,向邮件提供商发起邮件服务请求
///邮件服务商完成相关的邮件服务,并返回结果
pub struct EmailClient {
    sender: SubscriberEmail,             //发送者的邮件地址
    http_client: Client,                 //作为客户端，与Postmark建立的连接
    base_url: String,                    //邮件服务商，如Postmark的API 根地址
    authorization_token: Secret<String>, //授权令牌
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        authorization_token: Secret<String>,
        time_out: std::time::Duration, //设置默认的超时时间,通过配置读入
    ) -> Self {
        //设置默认的超时时间
        let builder = Client::builder().timeout(time_out); //调用ClientBuilder来配置Client
        Self {
            sender,
            http_client: builder.build().unwrap(),
            base_url,
            authorization_token,
        }
    }

    ///发送给订阅者邮件
    pub async fn send_email(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/email", self.base_url);
        let request_body = SendEmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject: subject,
            html_body: html_content,
            text_body: text_content,
        };
        let outcome = self
            .http_client
            .post(&url)
            .header(
                "X-Postmark-Server-Token",
                self.authorization_token.expose_secret(),
            )
            .json(&request_body) //自动添加header， "Content-type" "application/json"
            .send()
            .await?
            .error_for_status()?; //当response的状态码有问题的时候，返回err
        Ok(())
    }
}

///信的请求内容的json结构体
#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")] //用于将结构体的字段名重名为大写驼峰式（首字母大写）HtmlBody
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str,
}

#[cfg(test)]
mod tests {
    use crate::{domain::SubscriberEmail, email_client::EmailClient};
    use actix_web::test;
    use claim::{assert_err, assert_ok};
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker, faker::internet::en::SafeEmail};
    use secrecy::Secret;
    use std::time::Duration;
    use wiremock::Match;
    use wiremock::Request;
    use wiremock::matchers::{any, header, header_exists, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct SendEmailBodyMatcher;

    ///手动实现body_json的匹配
    impl Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            //尝试将请求体解析为JSON
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                dbg!(&body);
                //检查是否填充了所有的必填字段
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("TextBody").is_some()
                    && body.get("HtmlBody").is_some()
            } else {
                //解析失败，则不匹配请求
                false
            }
        }
    }

    ///生成随机的邮件主题
    fn subject() -> String {
        Sentence(1..2).fake()
    }

    ///生成随机的邮件内容
    fn content() -> String {
        Paragraph(1..10).fake()
    }

    ///生成随机的电子邮件地址
    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    ///获取`emailclient` 的实例
    fn email_client(base_url: String) -> EmailClient {
        //设置timeout短一点，方便测试
        EmailClient::new(
            base_url,
            email(),
            Secret::new(Faker.fake()),
            std::time::Duration::from_millis(200),
        )
    }

    ///服务器返回正确
    #[tokio::test]
    async fn send_email_succeeds_if_the_server_returns_200() {
        //期望发送邮件到base_url
        let mock_server = MockServer::start().await; //完整的Http服务器,使用一个随机可用的端口
        let email_client = email_client(mock_server.uri()); //将MockServer的URL传递

        //尝试使用最少的内容，测试能否正常匹配
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        //执行
        let outcome = email_client
            .send_email(&email(), &subject(), &content(), &content())
            .await;

        //断言
        assert_ok!(outcome);
    }

    ///服务器返回错误
    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        //期望发送邮件到base_url
        let mock_server = MockServer::start().await; //完整的http服务器,使用一个随机可用的端口
        let email_client = email_client(mock_server.uri()); //将mockserver的url传递

        // 加入MockServer的mock行为
        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1) //表示测试期间，应该仅收到一个匹配的请求
            .mount(&mock_server)
            .await;

        //执行
        let outcome = email_client
            .send_email(&email(), &subject(), &content(), &content())
            .await;

        //断言
        assert_err!(outcome);
    }

    ///服务器超时错误
    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        //期望发送邮件到base_url
        let mock_server = MockServer::start().await; //完整的http服务器,使用一个随机可用的端口
        let email_client = email_client(mock_server.uri()); //将mockserver的url传递

        //Mockserver 用于回复的response
        let response = ResponseTemplate::new(200).set_delay(Duration::from_secs(180));

        // 加入MockServer的mock行为
        Mock::given(any())
            .respond_with(response)
            .expect(1) //表示测试期间，应该仅收到一个匹配的请求
            .mount(&mock_server)
            .await;

        //执行
        let outcome = email_client
            .send_email(&email(), &subject(), &content(), &content())
            .await;

        //断言
        assert_err!(outcome);
    }
}
