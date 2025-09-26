//! 测试侧简化 i18n 工具：提供最小 translate()/locale_keys() 能力
//! 仅用于单元测试，不绑定生产 i18n 资源，防止引入跨 crate 依赖。

use std::collections::HashMap;

fn fixture() -> HashMap<&'static str, (&'static str, &'static str)> {
    let mut m = HashMap::new();
    m.insert("error.network.timeout", ("Network timeout", "网络超时"));
    m.insert("error.protocol.invalid", ("Protocol invalid", "协议错误"));
    m.insert(
        "error.cancel.requested",
        ("Operation cancelled", "操作已取消"),
    );
    m
}

/// 返回所有可用的 i18n key（来自测试侧 fixture）。
pub fn locale_keys() -> Vec<&'static str> {
    fixture().keys().cloned().collect()
}

/// 简化翻译：目前支持 en/zh；其它 locale 回退到 en。缺失 key 返回 None。
pub fn translate(key: &str, locale: &str) -> Option<String> {
    let f = fixture();
    f.get(key).map(|(en, zh)| match locale {
        "zh" => zh.to_string(),
        _ => en.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_translate_and_keys() {
        assert!(locale_keys().contains(&"error.network.timeout"));
        assert_eq!(
            translate("error.network.timeout", "en").as_deref(),
            Some("Network timeout")
        );
        assert_eq!(
            translate("error.network.timeout", "zh").as_deref(),
            Some("网络超时")
        );
        assert_eq!(
            translate("error.network.timeout", "fr").as_deref(),
            Some("Network timeout")
        );
        assert!(translate("missing.key", "en").is_none());
    }
}
