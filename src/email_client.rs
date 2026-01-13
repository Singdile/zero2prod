//! src/email_client.rs

use crate::domain::SubscriberEmail;
use reqwest::Client;
///邮件客户端,将状态存储到数据结构，将行为放在impl实现
pub struct EmailClient {
    sender: SubscriberEmail, //发送者的邮件地址
    http_client: Client,     //作为客户端，与Postmark建立的连接
    base_url: String,        //邮件服务商，如Postmark的API 根地址
}

impl EmailClient {
    pub fn new(base_url: String, sender: SubscriberEmail) -> Self {
        Self {
            sender,
            http_client: Client::new(),
            base_url,
        }
    }

    ///发送给订阅者邮件
    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::SubscriberEmail,
        email_client::{self, EmailClient},
    };
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, faker::internet::en::SafeEmail};
    use wiremock::matchers::any;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {
        //期望发送邮件到base_url
        let mock_server = MockServer::start().await; //完整的Http服务器,使用一个随机可用的端口
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let email_client = EmailClient::new(mock_server.uri(), sender); //将MockServer的URL传递

        //加入MockServer的mock行为，当收到任何的请求都匹配，并且返回200的状态
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1) //表示测试期间，应该仅收到一个匹配的请求
            .mount(&mock_server)
            .await;

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();

        //执行
        let _ = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;

        //断言判断
    }
}
