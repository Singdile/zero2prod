//! src/configuration.rs

use secrecy::ExposeSecret;
use secrecy::Secret;

//默认实现序列化之后，serde会为结构体自动生成一套填充逻辑，会拿YAML里的Key去匹配结构体的字段名Field
#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>, //敏感信息，包装起来
    pub port: u16,
    pub host: String,
    pub database_name: String,
}

#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application_port: u16,
}

///获取数据库配置信息
pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    //初始化一个配置读取器
    let settings = config::Config::builder()
        .add_source(config::File::new(
            "configuration.yaml",
            config::FileFormat::Yaml,
        ))
        .build()?;

    //尝试将读取到的配置转换为Settings类型
    settings.try_deserialize::<Settings>()
}

impl DatabaseSettings {
    ///PostgreSQL 支持像浏览器地址一样的 URL 格式,
    /// 用于连接数据库,psql postgresql://<用户名>:<密码>@<主机地址>:<端口>/<数据库名>
    pub fn connection_string(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password.expose_secret(), //敏感数据开锁使用
            self.host,
            self.port,
            self.database_name
        ))
    }
    ///返回数据库连接字符串,但是省略了数据库名
    pub fn connection_string_without_db(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}",
            self.username,
            self.password.expose_secret(), //敏感数据开锁使用
            self.host,
            self.port,
        ))
    }
}
