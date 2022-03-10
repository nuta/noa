#[macro_use]
extern crate log;

use std::{
    env::current_dir,
    error::Error,
    path::{Path, PathBuf},
};

mod detect_indent;

pub use detect_indent::detect_indent_style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Tab,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndOfLine {
    Cr,
    Lf,
    CrLf,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorConfig {
    pub indent_style: IndentStyle,
    pub indent_size: usize,
    pub tab_width: usize,
    // TODO: Implement `end_of_line`.
    pub end_of_line: EndOfLine,
    pub insert_final_newline: bool,
}

impl EditorConfig {
    pub fn resolve_or_guess(source_file: &Path) -> EditorConfig {
        EditorConfig::resolve(source_file).unwrap_or_else(|| {
            read_to_string_4k(source_file)
                .ok()
                .and_then(|text| detect_indent_style(&text))
                .map(|(indent_style, indent_size)| EditorConfig {
                    indent_style,
                    indent_size,
                    ..Default::default()
                })
                .unwrap_or_default()
        })
    }

    pub fn resolve(source_file: &Path) -> Option<EditorConfig> {
        current_dir()
            .ok()
            .and_then(|cwd| resolve_config(&cwd.join(source_file)))
    }
}

impl Default for EditorConfig {
    fn default() -> EditorConfig {
        EditorConfig {
            indent_style: IndentStyle::Space,
            indent_size: 4,
            tab_width: 8,
            end_of_line: EndOfLine::Lf,
            insert_final_newline: false,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
struct Rule {
    pattern: String,
    indent_style: Option<IndentStyle>,
    indent_size: Option<usize>,
    tab_width: Option<usize>,
    end_of_line: Option<EndOfLine>,
    insert_final_newline: Option<bool>,
}

#[derive(Debug, PartialEq, Eq)]
struct ConfigFile {
    root: bool,
    rules: Vec<Rule>,
}

fn parse_config(body: &str) -> ConfigFile {
    let mut root = false;
    let mut rules = Vec::new();
    let mut rule: Rule = Default::default();
    let mut pattern = None;

    for mut line in body.split('\n') {
        line = line.trim_start();
        if line.starts_with('[') {
            // [pattern]
            if let Some(index) = line.find(']') {
                if let Some(pattern) = pattern {
                    rule.pattern = pattern;
                    rules.push(rule);
                }
                pattern = Some(line[1..index].to_string());
                rule = Default::default();
            }
        } else if line.starts_with('#') {
            // A comment line. Just ignore it.
        } else if let Some(index) = line.find('=') {
            // key = value
            if index < line.len() {
                let key = line[..index].trim();
                let mut value = line[(index + 1)..].trim();

                // Remove a comment.
                if let Some(index) = value.find('#') {
                    value = &value[..index - 1];
                }

                match key {
                    "root" => {
                        root = value == "true";
                    }
                    "insert_final_newline" => {
                        rule.insert_final_newline = Some(value == "true");
                    }
                    "indent_style" => match value {
                        "space" => {
                            rule.indent_style = Some(IndentStyle::Space);
                        }
                        "tab" => {
                            rule.indent_style = Some(IndentStyle::Tab);
                        }
                        _ => {}
                    },
                    "end_of_line" => match value {
                        "cr" => {
                            rule.end_of_line = Some(EndOfLine::Cr);
                        }
                        "lf" => {
                            rule.end_of_line = Some(EndOfLine::Lf);
                        }
                        "crlf" => {
                            rule.end_of_line = Some(EndOfLine::CrLf);
                        }
                        _ => {}
                    },
                    "indent_size" => {
                        if let Ok(value) = value.parse::<usize>() {
                            rule.indent_size = Some(value);
                        }
                    }
                    "tab_width" => {
                        if let Ok(value) = value.parse::<usize>() {
                            rule.tab_width = Some(value);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(pattern) = pattern {
        rule.pattern = pattern;
        rules.push(rule);
    }

    ConfigFile { root, rules }
}

fn skip(s: &str, n: usize) -> &str {
    if s.len() <= n {
        ""
    } else {
        &s[n..]
    }
}

fn matches_pattern(pattern: &str, path: &str) -> bool {
    if pattern.is_empty() && path.is_empty() {
        return true;
    }

    if pattern.is_empty() || path.is_empty() {
        return false;
    }

    if pattern.starts_with("**") {
        return matches_pattern(pattern, skip(path, 1))
            || matches_pattern(skip(pattern, 2), skip(path, 1))
            || matches_pattern(skip(pattern, 2), path);
    } else if pattern.starts_with('*') {
        if path.starts_with('/') {
            return matches_pattern(skip(pattern, 1), path);
        } else {
            return matches_pattern(pattern, skip(path, 1))
                || matches_pattern(skip(pattern, 1), skip(path, 1))
                || matches_pattern(skip(pattern, 1), path);
        }
    } else if pattern.starts_with('{') && pattern.contains('}') {
        let mut prev_comma = 0;
        let remaining = skip(pattern, pattern.find('}').unwrap() + 1);
        while let Some(mut comma) = skip(pattern, prev_comma + 1).find(|c| c == ',' || c == '}') {
            comma += prev_comma + 1;
            let inner_pattern = &pattern[(prev_comma + 1)..comma];
            let pat = format!("{}{}", inner_pattern, remaining);
            if matches_pattern(&pat, path) {
                return true;
            }

            if pattern[comma..].starts_with('}') {
                break;
            }

            prev_comma = comma;
        }
        false
    } else {
        // Skip until a special character.
        let mut i = 0;
        for ch in pattern.chars() {
            if ch == '{' || ch == '*' || ch == ']' {
                break;
            }
            i += 1;
        }

        let span = &pattern[0..i];
        if i > 0 && path.starts_with(span) {
            return matches_pattern(skip(pattern, i), skip(path, i));
        } else {
            false
        }
    }
}

fn resolve_config(source_file: &Path) -> Option<EditorConfig> {
    assert!(source_file.is_absolute());

    // Read and parse all .editconfig files...
    let mut configs = Vec::new();
    for dir in source_file.parent().unwrap().ancestors() {
        let mut path = PathBuf::from(dir);
        path.push(".editorconfig");
        if let Ok(mut file) = std::fs::File::open(&path) {
            use std::io::Read;
            let mut body = String::with_capacity(512);
            file.read_to_string(&mut body).ok();

            let config = parse_config(&body);
            let is_root = config.root;
            configs.push((dir.to_path_buf(), config));

            if is_root {
                break;
            }
        }
    }

    // Visit from the root and determine the config for the source file.
    let mut ret = EditorConfig::default();
    let mut matched_any = false;
    trace!("config: {:#?}", configs);
    for (dir, config) in configs.iter().rev() {
        for rule in &config.rules {
            let relative_path = if rule.pattern.starts_with('*') {
                // Handle [*.foo] patterns (i.e. matching all files in all dirs)
                source_file.file_name().unwrap().to_str().unwrap()
            } else {
                source_file.strip_prefix(dir).unwrap().to_str().unwrap()
            };

            if matches_pattern(&rule.pattern, relative_path) {
                trace!("applying {}/.editorconfig", dir.as_path().display());
                trace!("rule: {:#?}", rule);
                ret.indent_style = rule.indent_style.unwrap_or(ret.indent_style);
                ret.indent_size = rule.indent_size.unwrap_or(ret.indent_size);
                ret.tab_width = rule.tab_width.unwrap_or(ret.tab_width);
                ret.insert_final_newline = rule
                    .insert_final_newline
                    .unwrap_or(ret.insert_final_newline);

                matched_any = true;
            }
        }
    }

    if matched_any {
        Some(ret)
    } else {
        None
    }
}

fn read_to_string_4k(path: &Path) -> Result<String, Box<dyn Error>> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0; 4096];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    let str = String::from_utf8(buf)?;
    Ok(str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_config() {
        assert_eq!(
            parse_config(
                r#"
                # this is a comment
                root = true
                insert_final_newline = true
                indent_style = space # comment

                [*.rs] # comment
                indent_size = 4 # comment

                [*.md]
                indent_style = tab
                tab_width = 8
                end_of_line = crlf
                insert_final_newline = false

                [broken]
                foo =
                bar
            "#
            ),
            ConfigFile {
                root: true,
                rules: vec![
                    Rule {
                        pattern: "*.rs".to_owned(),
                        indent_style: None,
                        indent_size: Some(4),
                        tab_width: None,
                        end_of_line: None,
                        insert_final_newline: None,
                    },
                    Rule {
                        pattern: "*.md".to_owned(),
                        indent_style: Some(IndentStyle::Tab),
                        indent_size: None,
                        tab_width: Some(8),
                        end_of_line: Some(EndOfLine::CrLf),
                        insert_final_newline: Some(false),
                    },
                    Rule {
                        pattern: "broken".to_owned(),
                        indent_style: None,
                        indent_size: None,
                        tab_width: None,
                        end_of_line: None,
                        insert_final_newline: None,
                    }
                ]
            }
        );
    }

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern("lib/bar/baz.js", "lib/bar/baz.js"));
        assert!(matches_pattern("*", "foo.js"));
        assert!(!matches_pattern("*.js", "lib/foo.js"));
        assert!(matches_pattern("**", "lib/foo.js"));
        assert!(matches_pattern("lib/**", "lib/foo.js"));
        assert!(matches_pattern("lib/**.js", "lib/foo/bar.js"));
        assert!(!matches_pattern("lib/**.js", "lib/foo.rb"));
        assert!(matches_pattern("lib/*.js", "lib/foo.js"));
        assert!(!matches_pattern("lib/*.js", "lib/foo/bar.js"));
        assert!(matches_pattern("lib/**.{js,rb}", "lib/foo.rb"));
        assert!(!matches_pattern("lib/**.{js,rb}", "lib/foo.r"));
        assert!(!matches_pattern("lib/**.{js,rb}{a,b}", "lib/foo.rb"));
        assert!(matches_pattern("lib/**.{js,rb}{a,b}", "lib/foo.rba"));
        assert!(matches_pattern("{Makefile,.lldbrc,README.md}", "Makefile"));
        assert!(matches_pattern("{Makefile,.lldbrc,README.md}", ".lldbrc"));
        assert!(matches_pattern("{Makefile,.lldbrc,README.md}", "README.md"));

        assert!(!matches_pattern("", "foo.js"));
        assert!(!matches_pattern("{}", "foo.js"));
        assert!(!matches_pattern("{,,,}", "foo.js"));
        assert!(!matches_pattern("{,,,", "foo.js"));
        assert!(!matches_pattern("{", "foo.js"));
        assert!(!matches_pattern("}", "foo.js"));
    }
}
