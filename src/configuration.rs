//! src/configuration.rs
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::ConnectOptions;
use sqlx::postgres::PgConnectOptions;
use sqlx::postgres::PgSslMode; //处理加密通信
//默认实现序列化之后，serde会为结构体自动生成一套填充逻辑，会拿YAML里的Key去匹配结构体的字段名Field

///数据库配置信息
#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>, //敏感信息，包装起来

    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub database_name: String,
    pub require_ssl: bool, //确定是否需要加密连接
}

///应用程序配置信息
#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
}

///数据库配置信息和应用程序配置信息汇总
#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
}

///获取配置信息，加载Settings
pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    //工作目录的路径
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("configuration"); //配置目录的路径

    //检查运行环境
    // 如果没有指定的话，则默认是`local`
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    //local.yaml,production.yaml
    let environment_filename = format!("{}.yaml", environment.as_str());

    //加载配置文件，base.yaml 是通用的配置， environment 是具体的配置
    // 如果有重复的话，后面的配置会覆盖前面的配置
    let settings = config::Config::builder()
        .add_source(config::File::from(
            configuration_directory.join("base.yaml"),
        ))
        .add_source(config::File::from(
            configuration_directory.join(&environment_filename),
        ))
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize::<Settings>()
}

impl DatabaseSettings {
    ///PostgreSQL 支持像浏览器地址一样的 URL 格式,
    /// 用于连接数据库,psql postgresql://<用户名>:<密码>@<主机地址>:<端口>/<数据库名>
    pub fn with_db(&self) -> PgConnectOptions {
        let mut options = self.without_db().database(&self.database_name);
        //设置sqlx的日志级别
        options.log_statements(tracing::log::LevelFilter::Trace);
        options
    }
    ///返回数据库连接字符串,但是省略了数据库名
    pub fn without_db(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require //仅尝试ssl连接
        } else {
            PgSslMode::Prefer // 先尝试ssl连接，不行的话再尝试无ssl连接
        };

        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(&self.password.expose_secret())
            .port(self.port)
            .ssl_mode(ssl_mode)
    }
}

/// 应用程序可能的运行时环境
pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a surpported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}
