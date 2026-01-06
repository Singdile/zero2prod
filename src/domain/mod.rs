//! src/domain/mod.rs
//! 将订阅者的实现逻辑拆分

//! 类型驱动开发
//! 需求：所有的订阅者姓名都必须符合一些约束条件
//! 潜在的问题：在调用 insert_subscriber(form: &FormData, pool: &PgPool)函数之前可能会忘记验证输入的订阅者姓名
//! 解决方法：新定义一个类型SubscriberName, 该类型的实例一定是非空的。 新定义一个类型: NewSubscriber——包含SubscriberName; 更改insert_subscriber(form: &NewSubscriber, pool: &PgPool)
//! 如此，只要通过编译，就一定能保证使用的数据的名称是经过验证的

mod new_subscriber;
mod subscriber_email;
mod subscriber_name;

//扁平化,当引入domain模块的时候，可以直接domain::Newsubscriber
pub use new_subscriber::NewSubscriber;
pub use subscriber_email::SubscriberEmail;
pub use subscriber_name::SubscriberName;
