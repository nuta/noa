use regex::Regex;

pub enum SearchQuery {
    MatchAll,
    MatchNone,
    Plain(String),
    Regex(Regex),
}
