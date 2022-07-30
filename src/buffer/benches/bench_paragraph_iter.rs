#![feature(test)]

extern crate test;

use noa_buffer::cursor::*;
use noa_buffer::raw_buffer::*;

#[bench]
fn bench_iterate_next_paragraph(b: &mut test::Bencher) {
    let buffer = RawBuffer::from_text(
        &"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n"
            .repeat(256),
    );

    b.iter(|| {
        let mut iter = buffer.paragraph_iter(Position::new(0, 0), 60, 4);
        for paragraph in iter.by_ref() {
            test::black_box(paragraph);
        }
    })
}
