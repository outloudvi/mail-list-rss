use mail_parser::Message;
use serde::Deserialize;

use crate::db::ToVec;

#[derive(Clone, Debug, Deserialize)]
pub struct Rule {
    pub to_box: String,
    pub filter: Vec<RuleFilter>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum RuleFilter {
    ByFrom(String),
    ByTo(String),
}

impl RuleFilter {
    pub fn matches(&self, msg: &Message) -> bool {
        let mut compare_item = match self {
            RuleFilter::ByFrom(_) => msg.get_from().to_vec(),
            RuleFilter::ByTo(_) => msg.get_to().to_vec(),
        };
        let base = match self {
            RuleFilter::ByFrom(x) => x,
            RuleFilter::ByTo(x) => x,
        };
        compare_item.sort();
        compare_item.iter().filter(|x| x == &base).next().is_some()
    }
}

mod test {
    #[test]
    fn test_deserialize() {
        let rule = r#"{
    "to_box": "a@example.com",
    "filter": [
        {"type": "ByFrom","params": "b@example.com"},
        {"type": "ByTo","params": "c@example.com"}
    ]
}"#;
        let result: Rule = from_str(rule).unwrap();
        assert_eq!(result.to_box, Some("a@example.com".to_owned()));
        match result.filter.first().unwrap() {
            RuleFilter::ByFrom(x) => assert_eq!(x, "b@example.com"),
            RuleFilter::ByTo(_) => unreachable!(),
        }
        match result.filter.last().unwrap() {
            RuleFilter::ByTo(x) => assert_eq!(x, "c@example.com"),
            RuleFilter::ByFrom(_) => unreachable!(),
        }
    }
}
