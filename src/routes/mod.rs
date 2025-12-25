mod health_check;
mod subscriptions;

pub use health_check::*;
pub use subscriptions::*;

// 这两行是“扁平化”API 的常用技巧。

// 作用：它们把子模块里的所有内容（pub 的部分）直接拉到了当前模块（routes）的层级。

// 带来的好处：

// 外部调用更简单：原本外部需要写 use crate::routes::health_check::health_check_handler;。

// 现在只需写：use crate::routes::health_check_handler;。

// 它隐藏了内部复杂的文件目录结构，给外部调用者提供了一个干净、简洁的接口。
