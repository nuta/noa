use noa_buffer::cursor::*;
use noa_buffer::raw_buffer::*;

#[test]
fn test_substr() {
    let buffer = RawBuffer::from_text("...AB...");
    assert_eq!(buffer.substr(Range::new(0, 3, 0, 5)), "AB");

    let buffer = RawBuffer::from_text("あいうABえお");
    assert_eq!(buffer.substr(Range::new(0, 3, 0, 5)), "AB");
}

#[test]
fn test_char_iter() {
    let buffer = RawBuffer::from_text("XY\n123");
    let mut iter = buffer.char_iter(Position::new(1, 1));
    assert_eq!(iter.next(), Some('2'));
    assert_eq!(iter.prev(), Some('2'));
    assert_eq!(iter.prev(), Some('1'));
    assert_eq!(iter.prev(), Some('\n'));
    assert_eq!(iter.prev(), Some('Y'));
    assert_eq!(iter.prev(), Some('X'));
    assert_eq!(iter.prev(), None);
    assert_eq!(iter.next(), Some('X'));
    assert_eq!(iter.next(), Some('Y'));
    assert_eq!(iter.next(), Some('\n'));
    assert_eq!(iter.next(), Some('1'));

    let buffer = RawBuffer::from_text("XYZ");
    let mut iter = buffer.char_iter(Position::new(0, 1));
    assert_eq!(iter.prev(), Some('X'));
}

#[test]
fn test_grapheme_iter_next() {
    let buffer = RawBuffer::from_text("ABC");
    let mut iter = buffer.grapheme_iter(Position::new(0, 0));
    assert_eq!(iter.next().map(|s| s.to_string()), Some("A".to_string()));
    assert_eq!(iter.next().map(|s| s.to_string()), Some("B".to_string()));
    assert_eq!(iter.next().map(|s| s.to_string()), Some("C".to_string()));
    assert_eq!(iter.next().map(|s| s.to_string()), None);

    let buffer = RawBuffer::from_text("あaい");
    let mut iter = buffer.grapheme_iter(Position::new(0, 0));
    assert_eq!(iter.next().map(|s| s.to_string()), Some("あ".to_string()));
    assert_eq!(iter.next().map(|s| s.to_string()), Some("a".to_string()));
    assert_eq!(iter.next().map(|s| s.to_string()), Some("い".to_string()));
    assert_eq!(iter.next().map(|s| s.to_string()), None);

    // A grapheme ("ka" in Katakana with Dakuten), consists of U+304B U+3099.
    let buffer = RawBuffer::from_text("\u{304b}\u{3099}");
    let mut iter = buffer.grapheme_iter(Position::new(0, 0));
    assert_eq!(
        iter.next().map(|s| s.to_string()),
        Some("\u{304b}\u{3099}".to_string())
    );
    assert_eq!(iter.next().map(|s| s.to_string()), None);

    // A grapheme ("u" with some marks), consists of U+0075 U+0308 U+0304.
    let buffer = RawBuffer::from_text("\u{0075}\u{0308}\u{0304}BC");
    let mut iter = buffer.grapheme_iter(Position::new(0, 0));
    assert_eq!(
        iter.next().map(|s| s.to_string()),
        Some("\u{0075}\u{0308}\u{0304}".to_string())
    );
    assert_eq!(iter.next().map(|s| s.to_string()), Some("B".to_string()));
    assert_eq!(iter.next().map(|s| s.to_string()), Some("C".to_string()));
    assert_eq!(iter.next().map(|s| s.to_string()), None);
}

#[test]
fn test_grapheme_iter_prev() {
    let buffer = RawBuffer::from_text("ABC");
    let mut iter = buffer.grapheme_iter(Position::new(0, 3));
    assert_eq!(iter.prev().map(|s| s.to_string()), Some("C".to_string()));
    assert_eq!(iter.prev().map(|s| s.to_string()), Some("B".to_string()));
    assert_eq!(iter.prev().map(|s| s.to_string()), Some("A".to_string()));
    assert_eq!(iter.prev().map(|s| s.to_string()), None);

    let buffer = RawBuffer::from_text("あaい");
    let mut iter = buffer.grapheme_iter(Position::new(0, 3));
    assert_eq!(iter.prev().map(|s| s.to_string()), Some("い".to_string()));
    assert_eq!(iter.prev().map(|s| s.to_string()), Some("a".to_string()));
    assert_eq!(iter.prev().map(|s| s.to_string()), Some("あ".to_string()));
    assert_eq!(iter.prev().map(|s| s.to_string()), None);

    // A grapheme ("か" with dakuten), consists of U+304B U+3099.
    let buffer = RawBuffer::from_text("\u{304b}\u{3099}");
    let mut iter = buffer.grapheme_iter(Position::new(0, 2));
    assert_eq!(
        iter.prev().map(|s| s.to_string()),
        Some("\u{304b}\u{3099}".to_string())
    );
    assert_eq!(iter.prev().map(|s| s.to_string()), None);

    // A grapheme ("u" with some marks), consists of U+0075 U+0308 U+0304.
    let buffer = RawBuffer::from_text("\u{0075}\u{0308}\u{0304}BC");
    let mut iter = buffer.grapheme_iter(Position::new(0, 5));
    assert_eq!(iter.prev().map(|s| s.to_string()), Some("C".to_string()));
    assert_eq!(iter.prev().map(|s| s.to_string()), Some("B".to_string()));
    assert_eq!(
        iter.prev().map(|s| s.to_string()),
        Some("\u{0075}\u{0308}\u{0304}".to_string())
    );
    assert_eq!(iter.prev().map(|s| s.to_string()), None);
}

#[test]
fn test_word() {
    let buffer = RawBuffer::from_text("");
    let mut iter = buffer.word_iter(Position::new(0, 0));
    assert_eq!(iter.next().map(|w| w.range()), None);

    let buffer = RawBuffer::from_text("A");
    let mut iter = buffer.word_iter(Position::new(0, 0));
    assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 0, 0, 1)));
    assert_eq!(iter.next().map(|w| w.range()), None);

    let buffer = RawBuffer::from_text("ABC DEF");
    let mut iter = buffer.word_iter(Position::new(0, 3));
    assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 0, 0, 3)));

    let buffer = RawBuffer::from_text("abc WXYZ   12");
    let mut iter = buffer.word_iter(Position::new(0, 0));
    assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 0, 0, 3)));
    assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 4, 0, 8)));
    assert_eq!(
        iter.next().map(|w| w.range()),
        Some(Range::new(0, 11, 0, 13))
    );
    assert_eq!(iter.next().map(|w| w.range()), None);

    let mut iter = buffer.word_iter(Position::new(0, 5));
    assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 4, 0, 8)));

    let mut iter = buffer.word_iter(Position::new(0, 8));
    assert_eq!(iter.next().map(|w| w.range()), Some(Range::new(0, 4, 0, 8)));
}
