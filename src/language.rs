use regex::Regex;

pub struct Language {
    pub name: &'static str,
    pub keywords: Vec<Regex>,
    pub line_comments: Vec<Regex>,
}

// pub const PLAIN: Language = Language {
//     name: "plain",
//     keywords: Regex::new("")
// };

lazy_static! {
    pub static ref PLAIN: Language = {
        let keywords = vec![
            Regex::new(
                "if|for|while|do|goto|break|continue|case|default|return|switch"
            ).unwrap()
        ];
        let line_comments = vec![
            Regex::new(
                "//.*"
            ).unwrap()
        ];

        Language {
            name: "plain",
            keywords,
            line_comments,
        }
    };
}
