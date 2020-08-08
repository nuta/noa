use regex::Regex;

pub struct Language {
    pub name: &'static str,
    pub highlights: Vec<Regex>,
}

// pub const PLAIN: Language = Language {
//     name: "plain",
//     highlights: Regex::new("")
// };

lazy_static! {
    pub static ref PLAIN: Language = {
        let highlights = vec![
            Regex::new(
                "if|for|while|do|goto|break|continue|case|default|return|switch"
            ).unwrap()
        ];

        Language {
            name: "plain",
            highlights,
        }
    };
}
