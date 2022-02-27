use noa_languages::tree_sitter;

use crate::{buffer::Buffer, cursor::Range, syntax::TsNodeExt};

impl Buffer {
    pub fn expand_selections(&mut self) {
        self.update_cursors_with(|c, buf| {
            if let Some(syntax) = buf.syntax() {
                let root = syntax.tree().root_node();
                let new_selection = walk_ts_node(root, &mut root.walk(), c.selection());
                c.select_range(new_selection);
            }
        });
    }
}

fn walk_ts_node<'tree>(
    parent: tree_sitter::Node<'tree>,
    cursor: &mut tree_sitter::TreeCursor<'tree>,
    selection: Range,
) -> Range {
    dbg!(parent.kind());

    for node in parent.children(cursor) {
        let range = node.buffer_range();
        if range.contains_range(selection) && range != selection {
            // A child node may contain narrower selection than `range`.
            return walk_ts_node(node, &mut node.walk(), selection);
        }
    }

    parent.buffer_range()
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::cursor::Cursor;

    use super::*;
    use noa_languages::language::get_language_by_name;
    use pretty_assertions::assert_eq;

    fn selected_str(buf: &Buffer) -> Cow<'_, str> {
        Cow::from(buf.substr(buf.cursors()[0].selection()))
    }

    #[test]
    fn expand_selections() {
        let mut b = Buffer::from_text("");
        b.set_language(get_language_by_name("rust").unwrap());
        b.post_update_hook();
        b.set_cursors_for_test(&[Cursor::new(0, 0)]);
        b.expand_selections();
        assert_eq!(selected_str(&b), "");

        // source_file [0, 0] - [5, 0]
        //   function_item [0, 0] - [4, 1]
        //     visibility_modifier [0, 0] - [0, 3]
        //     name: identifier [0, 7] - [0, 11]
        //     parameters: parameters [0, 11] - [0, 13]
        //     body: block [0, 14] - [4, 1]
        //       if_expression [1, 4] - [3, 5]
        //         condition: boolean_literal [1, 7] - [1, 11]
        //         consequence: block [1, 12] - [3, 5]
        //           macro_invocation [2, 8] - [2, 32]
        //             macro: identifier [2, 8] - [2, 11]
        //             token_tree [2, 12] - [2, 32]
        //               identifier [2, 13] - [2, 16]
        //               token_tree [2, 17] - [2, 31]
        //                 integer_literal [2, 18] - [2, 21]
        //                 integer_literal [2, 24] - [2, 25]
        //                 integer_literal [2, 27] - [2, 30]
        let mut b = Buffer::from_text(concat!(
            "pub fn main() {\n",
            "    if true {\n",
            "        dbg!(vec![123 + 0, 456]);\n",
            "    }\n",
            "}\n",
        ));
        b.set_language(get_language_by_name("rust").unwrap());
        b.post_update_hook();

        // The cursor is located in "123".
        b.set_cursors_for_test(&[Cursor::new(2, 21)]);

        b.expand_selections();
        assert_eq!(selected_str(&b), "123");
        b.expand_selections();
        assert_eq!(selected_str(&b), "[123 + 0, 456]");
        b.expand_selections();
        assert_eq!(selected_str(&b), "(vec![123 + 0, 456])");
        b.expand_selections();
        assert_eq!(selected_str(&b), "dbg!(vec![123 + 0, 456])");
        b.expand_selections();
        assert_eq!(selected_str(&b), "dbg!(vec![123 + 0, 456]);");
        b.expand_selections();
        assert_eq!(
            selected_str(&b),
            "{\n        dbg!(vec![123 + 0, 456]);\n    }"
        );
        b.expand_selections();
        assert_eq!(
            selected_str(&b),
            "if true {\n        dbg!(vec![123 + 0, 456]);\n    }"
        );
        b.expand_selections();
        assert_eq!(
            selected_str(&b),
            "{\n    if true {\n        dbg!(vec![123 + 0, 456]);\n    }\n}"
        );
        b.expand_selections();
        assert_eq!(
            selected_str(&b),
            "pub fn main() {\n    if true {\n        dbg!(vec![123 + 0, 456]);\n    }\n}"
        );
        b.expand_selections();
        assert_eq!(
            selected_str(&b),
            "pub fn main() {\n    if true {\n        dbg!(vec![123 + 0, 456]);\n    }\n}\n"
        );
    }
}
