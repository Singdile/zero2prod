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
        todo!()
    }
}
