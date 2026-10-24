//! Object properties.

/// Object properties, all values stringified as in `info.props`.
pub type PropMap = std::collections::BTreeMap<String, String>;

/// Typed reads of a [`PropMap`]. String reads use the map's own `get`.
pub trait PropMapExt {
    /// `key` parsed as a decimal `u32`; missing or unparsable is `None`.
    fn get_u32(&self, key: &str) -> Option<u32>;
    /// `key` parsed as a decimal `i32`; missing or unparsable is `None`.
    fn get_i32(&self, key: &str) -> Option<i32>;
    /// Boolean property: `"true"` or `"1"` is `true`; anything else, or missing, is `false`.
    fn flag(&self, key: &str) -> bool;
}

impl PropMapExt for PropMap {
    fn get_u32(&self, key: &str) -> Option<u32> {
        self.get(key)?.parse().ok()
    }

    fn get_i32(&self, key: &str) -> Option<i32> {
        self.get(key)?.parse().ok()
    }

    fn flag(&self, key: &str) -> bool {
        self.get(key).is_some_and(|v| v == "true" || v == "1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> PropMap {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn flag_parses_true_false_and_missing() {
        let p = props(&[
            ("a", "true"),
            ("b", "false"),
            ("c", "1"),
            ("d", "0"),
            ("e", "yes"),
        ]);
        assert!(p.flag("a"));
        assert!(!p.flag("b"));
        assert!(p.flag("c"));
        assert!(!p.flag("d"));
        assert!(!p.flag("e"));
        assert!(!p.flag("missing"));
    }

    #[test]
    fn get_u32_rejects_garbage() {
        let p = props(&[
            ("ok", "47"),
            ("neg", "-1"),
            ("text", "abc"),
            ("space", " 4"),
            ("empty", ""),
            ("big", "4294967296"),
        ]);
        assert_eq!(p.get_u32("ok"), Some(47));
        assert_eq!(p.get_u32("neg"), None);
        assert_eq!(p.get_u32("text"), None);
        assert_eq!(p.get_u32("space"), None);
        assert_eq!(p.get_u32("empty"), None);
        assert_eq!(p.get_u32("big"), None);
        assert_eq!(p.get_u32("missing"), None);
        assert_eq!(p.get_i32("neg"), Some(-1));
        assert_eq!(p.get_i32("text"), None);
    }
}
