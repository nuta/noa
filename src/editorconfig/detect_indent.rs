use std::collections::HashMap;

use crate::IndentStyle;

pub fn detect_indent_style(text: &str) -> Option<(IndentStyle, usize)> {
    // This map holds the occurrences of differences in the indentation from the previous line.
    let mut occurences = HashMap::new();
    let mut prev = None;
    for line in text.split('\n') {
        let indent_char = match line.chars().next() {
            Some('\t') => '\t',
            Some(' ') => ' ',
            _ => continue,
        };
        let indent_count = line.chars().take_while(|&ch| ch == indent_char).count();

        let current = (indent_char, indent_count);
        if prev.is_none() || Some(current) != prev {
            let count_diff = match prev {
                Some(prev) if current.0 == prev.0 => {
                    ((current.1 as isize) - (prev.1 as isize)).abs() as usize
                }
                _ => current.1,
            };

            if count_diff > 0 {
                occurences
                    .entry((current.0, count_diff))
                    .and_modify(|count| *count += 1)
                    .or_insert(1usize);
            }
        }

        prev = Some(current);
    }

    occurences
        .into_iter()
        .max_by(|(_, a), (_, b)| a.cmp(b))
        .map(|((indent_char, indent_size), _)| {
            let style = match indent_char {
                '\t' => IndentStyle::Tab,
                _ => IndentStyle::Space,
            };
            (style, indent_size)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_cases() {
        assert_eq!(detect_indent_style(""), None);
        assert_eq!(detect_indent_style("int a;"), None);
    }

    #[test]
    fn guess_tab_indent() {
        assert_eq!(
            detect_indent_style(
                r#"
int main() {
	if () {
		printf();
		hello();
		world();
	}
}
"#,
            ),
            Some((IndentStyle::Tab, 1))
        );

        assert_eq!(
            detect_indent_style(
                r#"
int main() {
	printf("hi");
}
"#,
            ),
            Some((IndentStyle::Tab, 1))
        );
    }

    #[test]
    fn guess_2_spaces_indent() {
        assert_eq!(
            detect_indent_style(
                r#"
int main() {
  if () {
    printf();
    hello();
    world();
  }
}
"#,
            ),
            Some((IndentStyle::Space, 2))
        );

        assert_eq!(
            detect_indent_style(
                r#"
int main() {
  printf("hi");
}
"#,
            ),
            Some((IndentStyle::Space, 2))
        );
    }

    #[test]
    fn guess_4_spaces_indent() {
        assert_eq!(
            detect_indent_style(
                r#"
int main() {
    if () {
        printf();
        hello();
        world();
    }
}
"#,
            ),
            Some((IndentStyle::Space, 4))
        );

        assert_eq!(
            detect_indent_style(
                r#"
int main() {
    printf("hi");
}
"#,
            ),
            Some((IndentStyle::Space, 4))
        );
    }
}
