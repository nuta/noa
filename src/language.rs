use regex::RegexSet;

pub struct Language {
    pub name: &'static str,
    pub highlights: RegexSet,
}

// pub const PLAIN: Language = Language {
//     name: "plain",
//     highlights: RegexSet::new("")
// };

lazy_static! {
    pub static ref C: Language = {
        let highlights = RegexSet::new(&[
            "if|for|while|do|goto|break|continue|case|default|return|switch",
        ]).unwrap();

        Language {
            name: "c",
            highlights,
        }
    };
}
