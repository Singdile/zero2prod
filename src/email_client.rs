//! src/email_client.rs
use crate::domain::SubscriberEmail;
use reqwest::Client;
use secrecy::{ExposeSecret, Secret};
use serde;
///邮件客户端,将状态存储到数据结构，将行为放在impl实现
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
    ) -> Self {
        Self {
            sender,
            http_client: Client::new(),
            base_url,
            authorization_token,
        }
    }

    ///发送给订阅者邮件
    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
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
        let _ = self
            .http_client
            .post(&url)
            .header(
                "X-Postmark-Server-Token",
                self.authorization_token.expose_secret(),
            )
            .json(&request_body) //自动添加header， "Content-type" "application/json"
            .send()
            .await?;
        Ok(())
    }
}

///信的请求内容的json结构体
#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")] //用于将结构体的字段名重名为大写驼峰式（首字母大写）
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
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker, faker::internet::en::SafeEmail};
    use secrecy::Secret;
    use wiremock::Match;
    use wiremock::Request;
    use wiremock::matchers::{header, header_exists, method, path};
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
    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        //期望发送邮件到base_url
        let mock_server = MockServer::start().await; //完整的Http服务器,使用一个随机可用的端口
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let email_client = EmailClient::new(mock_server.uri(), sender, Secret::new(Faker.fake())); //将MockServer的URL传递

        //加入MockServer的mock行为
        Mock::given(header_exists("X-Postmark-Server-Token"))
            .and(header("Content-Type", "application/json"))
            .and(method("POST"))
            .and(SendEmailBodyMatcher) //使用自己定义的匹配，来检查请求的json
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
    }
}
