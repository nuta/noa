#![feature(test)]

extern crate test;

use noa_buffer::cursor::*;
use noa_buffer::raw_buffer::*;

#[bench]
fn bench_iterate_next_graphemes(b: &mut test::Bencher) {
    // A grapheme ("u" with some marks), consists of U+0075 U+0308 U+0304.
    let buffer = RawBuffer::from_text(&"\u{0075}\u{0308}\u{0304}".repeat(128));

    b.iter(|| {
        let mut iter = buffer.grapheme_iter(Position::new(0, 0));
        for grapheme in iter.by_ref() {
            test::black_box(grapheme);
        }
    });
}

#[bench]
fn bench_iterate_prev_graphemes(b: &mut test::Bencher) {
    // A grapheme ("u" with some marks), consists of U+0075 U+0308 U+0304.
    let buffer = RawBuffer::from_text(&"\u{0075}\u{0308}\u{0304}".repeat(128));

    b.iter(|| {
        let mut iter = buffer.grapheme_iter(Position::new(0, 3 * 128));
        while let Some(grapheme) = iter.prev() {
            test::black_box(grapheme);
        }
    });
}
