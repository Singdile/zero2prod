use tracing::Subscriber;
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt};
///将多个层次组合一起形成 tracing 的订阅器
pub fn get_sunscriber<Sink>(
    name: String,
    env_filter: String,
    sink: Sink, //表示日志数据的最终目的地
) -> impl Subscriber + Send + Sync
where
    Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static, //sink产生的Writer的生命周期肯定要比其短,'a 表示一次写入的时间(生命周期),for<'a> 表示任意的生命周期都能满足
{
    //实际的返回类型完整写出来很冗余，这里使用impl trait 来说明返回的类型，只要该类型实现了特征即可
    //设置过滤器
    let env_filiter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));

    //设置输出格式
    let formatting_layer = BunyanFormattingLayer::new(name, sink);

    //组合处理层，生成一个订阅器
    Registry::default()
        .with(env_filiter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}

///将一个订阅器设置为全局默认值，用于处理所有跨度数据
/// 该函数只可调用一次
pub fn init_subscriber(subsriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Failed to set logger"); //将actix-web的log信息也发送到tracing里面
    set_global_default(subsriber).expect("Failed to set subscriber"); //设置全局的订阅器
}
