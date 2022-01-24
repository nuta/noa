use pretty_assertions::assert_eq;

use noa_buffer::buffer::*;
use noa_buffer::cursor::*;
use noa_editorconfig::IndentStyle;

#[test]
fn test_line_len() {
    assert_eq!(Buffer::from_text("").line_len(0), 0);
    assert_eq!(Buffer::from_text("A").line_len(0), 1);
    assert_eq!(Buffer::from_text("A\n").line_len(0), 1);
    assert_eq!(Buffer::from_text("A\nBC").line_len(1), 2);
    assert_eq!(Buffer::from_text("A\nBC\n").line_len(1), 2);
}

#[test]
fn insertion_and_backspace() {
    let mut b = Buffer::new();
    b.backspace();
    b.insert("Hello");
    b.insert(" World?");
    assert_eq!(b.text(), "Hello World?");
    b.backspace();
    assert_eq!(b.text(), "Hello World");
    b.insert_char('!');
    assert_eq!(b.text(), "Hello World!");
}

#[test]
fn deletion() {
    // a|bc
    let mut b = Buffer::new();
    b.insert("abc");
    b.set_cursors(&[Cursor::new(0, 1)]);
    b.delete();
    assert_eq!(b.text(), "ac");
    assert_eq!(b.cursors(), &[Cursor::new(0, 1),]);

    // a|
    // b
    let mut b = Buffer::new();
    b.insert("a\nb");
    b.set_cursors(&[Cursor::new(0, 1)]);
    b.delete();
    assert_eq!(b.text(), "ab");
    assert_eq!(b.cursors(), &[Cursor::new(0, 1),]);
}

#[test]
fn delete_selection() {
    // ab|XY        ab|cd
    // Z|cd|   =>
    let mut b = Buffer::new();
    b.insert("abXY\nZcd");
    b.set_cursors(&[Cursor::new_selection(0, 2, 1, 1)]);
    b.delete();
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);
}

#[test]
fn multibyte_characters() {
    let mut b = Buffer::new();
    b.insert("Hello 世界!");
    b.set_cursors(&[Cursor::new(0, 7)]);
    assert_eq!(b.len_chars(), 9);

    // Hello 世|界! => Hello |界!
    b.backspace();
    assert_eq!(b.text(), "Hello 界!");
    // Hello 世|界! => Hell|界!
    b.backspace();
    b.backspace();
    assert_eq!(b.text(), "Hell界!");
    // Hello 世|界! => Hell|界!
    b.insert("o こんにちは 世");
    assert_eq!(b.text(), "Hello こんにちは 世界!");
}

#[test]
fn test_insertion_at_eof() {
    let mut b = Buffer::from_text("ABC");
    b.set_cursors(&[Cursor::new(0, 3)]);
    b.insert_char('\n');
    assert_eq!(b.text(), "ABC\n");
    assert_eq!(b.cursors(), &[Cursor::new(1, 0)]);

    let mut b = Buffer::from_text("");
    b.set_cursors(&[Cursor::new(0, 0)]);
    b.insert_char('A');
    assert_eq!(b.text(), "A");
    assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);
}

#[test]
fn test_multiple_cursors1() {
    // ABC
    // おは
    // XY
    let mut b = Buffer::from_text("ABC\nおは\nXY");
    b.set_cursors(&[Cursor::new(0, 1), Cursor::new(1, 1), Cursor::new(2, 1)]);
    b.insert("!");
    assert_eq!(b.text(), "A!BC\nお!は\nX!Y");
    b.backspace();
    assert_eq!(b.text(), "ABC\nおは\nXY");
}

#[test]
fn test_multiple_cursors2() {
    // ABC
    // おは
    // XY
    let mut b = Buffer::from_text("ABC\nおは\nXY");
    b.set_cursors(&[
        Cursor::new_selection(0, 3, 1, 0),
        Cursor::new_selection(1, 2, 2, 0),
    ]);
    b.insert("!");
    assert_eq!(b.text(), "ABC!おは!XY");
    assert_eq!(b.cursors(), &[Cursor::new(0, 4), Cursor::new(0, 7)]);
}

#[test]
fn test_multiple_cursors3() {
    // A|B| => |
    let mut b = Buffer::from_text("AB");
    b.set_cursors(&[Cursor::new(0, 1), Cursor::new(0, 2)]);
    b.backspace();
    assert_eq!(b.text(), "");
    assert_eq!(b.cursors(), &[Cursor::new(0, 0)]);
}

#[test]
fn backspace_on_multi_cursors() {
    // abc|      ab|
    // def|  =>  de|
    // xyz|      xy|
    let mut b = Buffer::new();
    b.insert("abc\ndef\nxyz");
    b.set_cursors(&[Cursor::new(0, 3), Cursor::new(1, 3), Cursor::new(2, 3)]);
    b.backspace();
    assert_eq!(b.text(), "ab\nde\nxy");
    assert_eq!(
        b.cursors(),
        &[Cursor::new(0, 2), Cursor::new(1, 2), Cursor::new(2, 2),]
    );

    // abc|      ab|
    // 1|    =>  |
    // xy|z      x|z
    let mut b = Buffer::new();
    b.insert("abc\n1\nxyz");
    b.set_cursors(&[Cursor::new(0, 3), Cursor::new(1, 1), Cursor::new(2, 2)]);
    b.backspace();
    assert_eq!(b.text(), "ab\n\nxz");
    assert_eq!(
        b.cursors(),
        &[Cursor::new(0, 2), Cursor::new(1, 0), Cursor::new(2, 1),]
    );

    // 1230|a|b|c|d|e|f => 123|f
    let mut b = Buffer::new();
    b.insert("1230abcdef");
    b.set_cursors(&[
        Cursor::new(0, 4),
        Cursor::new(0, 5),
        Cursor::new(0, 6),
        Cursor::new(0, 7),
        Cursor::new(0, 8),
        Cursor::new(0, 9),
    ]);
    b.backspace();
    assert_eq!(b.text(), "123f");
    assert_eq!(b.cursors(), &[Cursor::new(0, 3)]);

    // a|bc      |bc|12
    // |12   =>  wxy|
    // wxyz|
    let mut b = Buffer::new();
    b.insert("abc\n12\nwxyz");
    b.set_cursors(&[Cursor::new(0, 1), Cursor::new(1, 0), Cursor::new(2, 4)]);
    b.backspace();
    assert_eq!(b.text(), "bc12\nwxy");
    assert_eq!(
        b.cursors(),
        &[Cursor::new(0, 0), Cursor::new(0, 2), Cursor::new(1, 3)]
    );

    // 0
    // |abc      0|abc|12|xyz
    // |12   =>
    // |xyz
    let mut b = Buffer::new();
    b.insert("0\nabc\n12\nxyz");
    b.set_cursors(&[Cursor::new(1, 0), Cursor::new(2, 0), Cursor::new(3, 0)]);
    b.backspace();
    assert_eq!(b.text(), "0abc12xyz");
    assert_eq!(
        b.cursors(),
        &[Cursor::new(0, 1), Cursor::new(0, 4), Cursor::new(0, 6),]
    );

    // ab|     =>  a|def|g
    // |c|def
    // |g
    let mut b = Buffer::new();
    b.insert("ab\ncdef\ng");
    b.set_cursors(&[
        Cursor::new(0, 2),
        Cursor::new(1, 0),
        Cursor::new(1, 1),
        Cursor::new(2, 0),
    ]);
    b.backspace();
    assert_eq!(b.text(), "adefg");
    assert_eq!(b.cursors(), &[Cursor::new(0, 1), Cursor::new(0, 4)]);

    // ab|   =>  a|def|g
    // |c|def
    // |g
    let mut b = Buffer::new();
    b.insert("ab\ncdef\ng");
    b.set_cursors(&[
        Cursor::new(0, 2),
        Cursor::new(1, 0),
        Cursor::new(1, 1),
        Cursor::new(2, 0),
    ]);
    b.backspace();
    assert_eq!(b.text(), "adefg");
    assert_eq!(b.cursors(), &[Cursor::new(0, 1), Cursor::new(0, 4)]);
}

#[test]
fn delete_on_multi_cursors() {
    // a|Xbc|Yd
    let mut b = Buffer::new();
    b.insert("aXbcYd");
    b.set_cursors(&[Cursor::new(0, 1), Cursor::new(0, 4)]);
    b.delete();
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.cursors(), &[Cursor::new(0, 1), Cursor::new(0, 3)]);

    // a|b|
    let mut b = Buffer::new();
    b.insert("ab");
    b.set_cursors(&[Cursor::new(0, 1), Cursor::new(0, 2)]);
    b.delete();
    assert_eq!(b.text(), "a");
    assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);

    // a|bc
    // d|ef
    // g|hi
    let mut b = Buffer::new();
    b.insert("abc\ndef\nghi");
    b.set_cursors(&[Cursor::new(0, 1), Cursor::new(1, 1), Cursor::new(2, 1)]);
    b.delete();
    assert_eq!(b.text(), "ac\ndf\ngi");
    assert_eq!(
        b.cursors(),
        &[Cursor::new(0, 1), Cursor::new(1, 1), Cursor::new(2, 1),]
    );

    // a|
    // b|X
    // c|Y
    // d|
    let mut b = Buffer::new();
    b.insert("a\nbX\ncY\nd");
    b.set_cursors(&[
        Cursor::new(0, 1),
        Cursor::new(1, 1),
        Cursor::new(2, 1),
        Cursor::new(3, 1),
    ]);
    b.delete();
    assert_eq!(b.text(), "ab\nc\nd");
    assert_eq!(
        b.cursors(),
        &[
            Cursor::new(0, 1),
            Cursor::new(0, 2),
            Cursor::new(1, 1),
            Cursor::new(2, 1),
        ]
    );

    // ab|
    // cde|
    let mut b = Buffer::new();
    b.insert("ab\ncde");
    b.set_cursors(&[Cursor::new(0, 2), Cursor::new(1, 3)]);
    b.delete();
    assert_eq!(b.text(), "abcde");
    assert_eq!(b.cursors(), &[Cursor::new(0, 2), Cursor::new(0, 5)]);

    // abc|
    // |d|ef
    // ghi|
    let mut b = Buffer::new();
    b.insert("abc\ndef\nghi");
    b.set_cursors(&[
        Cursor::new(0, 3),
        Cursor::new(1, 0),
        Cursor::new(1, 1),
        Cursor::new(2, 3),
    ]);
    b.delete();
    assert_eq!(b.text(), "abcf\nghi");
    assert_eq!(b.cursors(), &[Cursor::new(0, 3), Cursor::new(1, 3)]);

    // abc|     => abc|d|e|f
    // d|Xe|Yf
    let mut b = Buffer::new();
    b.insert("abc\ndXeYf");
    b.set_cursors(&[Cursor::new(0, 3), Cursor::new(1, 1), Cursor::new(1, 3)]);
    b.delete();
    assert_eq!(b.text(), "abcdef");
    assert_eq!(
        b.cursors(),
        &[Cursor::new(0, 3), Cursor::new(0, 4), Cursor::new(0, 5),]
    );
}

#[test]
fn multibyte_characters_regression1() {
    let mut b = Buffer::new();
    b.set_cursors(&[Cursor::new(0, 0)]);
    b.insert_char('a');
    b.insert_char('あ');
    b.insert_char('!');
    assert_eq!(b.text(), "aあ!");
}

#[test]
fn single_selection_including_newlines() {
    let mut b = Buffer::from_text("A\nB");
    b.set_cursors(&[Cursor::new_selection(0, 1, 1, 0)]);
    b.backspace();
    assert_eq!(b.text(), "AB");
    assert_eq!(b.cursors(), &[Cursor::new(0, 1)]);

    // xy|A     xy|z
    // BCD  =>
    // E|z
    let mut b = Buffer::new();
    b.insert("xyA\nBCD\nEz");
    b.set_cursors(&[Cursor::new_selection(0, 2, 2, 1)]);
    b.backspace();
    assert_eq!(b.text(), "xyz");
    assert_eq!(b.cursors(), &[Cursor::new(0, 2)]);

    // ab|      abX|c
    // |c   =>
    //
    let mut b = Buffer::new();
    b.insert("ab\nc");
    b.set_cursors(&[Cursor::new_selection(0, 2, 1, 0)]);
    b.insert("X");
    assert_eq!(b.text(), "abXc");
    assert_eq!(b.cursors(), &[Cursor::new(0, 3)]);
}

#[test]
fn multi_selections() {
    // ab|XYZ  =>  ab|
    // cd|XYZ  =>  cd|
    // ef|XYZ  =>  ef|
    let mut b = Buffer::new();
    b.insert("abXYZ\ncdXYZ\nefXYZ");
    b.set_cursors(&[
        Cursor::new_selection(0, 2, 0, 5),
        Cursor::new_selection(1, 2, 1, 5),
        Cursor::new_selection(2, 2, 2, 5),
    ]);
    b.delete();
    assert_eq!(b.text(), "ab\ncd\nef");
    assert_eq!(
        b.cursors(),
        &[Cursor::new(0, 2), Cursor::new(1, 2), Cursor::new(2, 2),]
    );

    // ab|XY        ab|cd|ef
    // Z|cd|XY  =>
    // Z|ef
    let mut b = Buffer::new();
    b.insert("abXY\nZcdXY\nZef");
    b.set_cursors(&[
        Cursor::new_selection(0, 2, 1, 1),
        Cursor::new_selection(1, 3, 2, 1),
    ]);
    b.backspace();
    assert_eq!(b.text(), "abcdef");
    assert_eq!(b.cursors(), &[Cursor::new(0, 2), Cursor::new(0, 4)]);

    // ab|XY        ab|cd|ef|g
    // Z|cd|XY  =>
    // Z|ef|XY
    // Z|g
    let mut b = Buffer::new();
    b.insert("abXY\nZcdXY\nZefXY\nZg");
    b.set_cursors(&[
        Cursor::new_selection(0, 2, 1, 1),
        Cursor::new_selection(1, 3, 2, 1),
        Cursor::new_selection(2, 3, 3, 1),
    ]);
    b.backspace();
    assert_eq!(b.text(), "abcdefg");
    assert_eq!(
        b.cursors(),
        &[Cursor::new(0, 2), Cursor::new(0, 4), Cursor::new(0, 6),]
    );
}

#[test]
fn test_insert_newline_and_indent() {
    let mut b = Buffer::from_text("");
    b.set_cursors(&[Cursor::new(0, 0)]);
    b.insert_newline_and_indent();
    assert_eq!(b.config().indent_style, IndentStyle::Space);
    assert_eq!(b.config().indent_size, 4);
    assert_eq!(b.text(), "\n");
    assert_eq!(b.cursors(), &[Cursor::new(1, 0)]);

    let mut b = Buffer::from_text("        abXYZ");
    b.set_cursors(&[Cursor::new(0, 10)]);
    b.insert_newline_and_indent();
    assert_eq!(b.text(), "        ab\n        XYZ");
    assert_eq!(b.cursors(), &[Cursor::new(1, 8)]);
}

#[test]
fn test_indent() {
    let mut b = Buffer::from_text("");
    b.set_cursors(&[Cursor::new(0, 0)]);
    b.indent();
    assert_eq!(b.config().indent_style, IndentStyle::Space);
    assert_eq!(b.config().indent_size, 4);
    assert_eq!(b.text(), "    ");

    //     abc
    let mut b = Buffer::from_text("    abc\n");
    b.set_cursors(&[Cursor::new(1, 0)]);
    b.indent();
    assert_eq!(b.text(), "    abc\n    ");

    // __
    let mut b = Buffer::from_text("  ");
    b.set_cursors(&[Cursor::new(0, 2)]);
    b.indent();
    assert_eq!(b.text(), "    ");

    // a
    let mut b = Buffer::from_text("a");
    b.set_cursors(&[Cursor::new(0, 1)]);
    b.indent();
    assert_eq!(b.text(), "a   ");

    // _____
    let mut b = Buffer::from_text("     ");
    b.set_cursors(&[Cursor::new(0, 5)]);
    b.indent();
    assert_eq!(b.text(), "        ");

    // if true {
    //     while true {
    let mut b = Buffer::from_text("if true {\n    while true {\n");
    b.set_cursors(&[Cursor::new(2, 0)]);
    b.indent();
    assert_eq!(b.text(), "if true {\n    while true {\n        ");

    // if true {
    //     while true {
    // __
    let mut b = Buffer::from_text("if true {\n    while true {\n  ");
    b.set_cursors(&[Cursor::new(2, 2)]);
    b.indent();
    assert_eq!(b.text(), "if true {\n    while true {\n        ");
}
