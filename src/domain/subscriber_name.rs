//! src/domain/subscriber_name.rs

use unicode_segmentation::UnicodeSegmentation;

///符合约束条件的订阅者姓名
#[derive(Debug)]
pub struct SubscriberName(String);

impl SubscriberName {
    ///如果输入满足订阅者姓名验证，则返回一个`Subscribername`实例
    /// 否则，抛出一个panic!
    pub fn parse(s: String) -> Result<SubscriberName, String> {
        //检查是否为空
        let is_empty_or_whitespace = s.trim().is_empty();

        //检查名字长度是否合法,graphemes()函数返回一个，
        // is_extend 参数表示能将多个unicode码组合的识别为一个视觉字符
        let is_too_long = s.graphemes(true).count() > 256;

        //遍历输入`s`中的所有字符，检查他们是否与禁用数组中的字符匹配
        let forbidden_characters = ['/', '(', ')', '\"', '<', '>', '\\', '{', '}'];
        let contains_forbidden_charaters = s.chars().any(|g| forbidden_characters.contains(&g)); //只要有一个true 就会直接返回

        //如果不满足任意一个条件则返回 `false`
        if is_empty_or_whitespace || is_too_long || contains_forbidden_charaters {
            Err(format!("{} is not a valid subscriber name.", s))
        } else {
            Ok(Self(s.to_string()))
        }
    }

    ///仅暴露值
    ///调用者获取内部字符串，但是不再拥有SubscriberName
    // String的所有权发生了变化，获取信息同时也无法更改信息了
    pub fn inner(self) -> String {
        self.0
    }

    ///暴露可变引用
    ///调用者获取内部字符串，返回了一个可变的引用,有可能改变信息
    pub fn inner_mut(&mut self) -> &mut str {
        &mut self.0
    }
}

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}



//单元测试
#[cfg(test)] //该行告诉编译器，cargo test 时才进行编译运行
mod tests {
    //测试专用模块,属于该文件模块的子模块，使用父模块的内容需要引入
    use crate::domain::SubscriberName;
    use claim::{assert_err, assert_ok};

    #[test] //以#[test] 标记的函数能被识别到
    fn a_256_grapheme_long_name_is_valid() {
        let name = "e".repeat(256);
        assert_ok!(SubscriberName::parse(name));
    }

    #[test] //以#[test] 标记的函数能被识别到
    fn a_name_longer_than_256_is_rejected() {
        let name = "e".repeat(257);
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn whitespace_only_names_are_rejected() {
        let name = "  ".to_string();
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn empty_string_is_rejected() {
        let name = String::new();
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn names_containing_an_invalid_character_are_rejected() {
        for name in &['/', '(', ')', '\"', '<', '>', '\\', '{', '}'] {
            let name = name.to_string();
            assert_err!(SubscriberName::parse(name));
        }
    }

    #[test]
    fn a_valid_name_is_parsed_successfully() {
        let name = "Singdile".to_string();
        assert_ok!(SubscriberName::parse(name));
    }
}
