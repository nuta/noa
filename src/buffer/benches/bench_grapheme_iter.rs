use criterion::{black_box, criterion_group, criterion_main, Criterion};

use noa_buffer::cursor::*;
use noa_buffer::raw_buffer::*;

fn benchmark(c: &mut Criterion) {
    // A grapheme ("u" with some marks), consists of U+0075 U+0308 U+0304.
    let buffer = RawBuffer::from_text(&"\u{0075}\u{0308}\u{0304}".repeat(128));

    c.bench_function("bench_iterate_next_graphemes", |b| {
        b.iter(|| {
            let mut iter = buffer.grapheme_iter(Position::new(0, 0));
            for grapheme in iter.by_ref() {
                black_box(grapheme);
            }
        })
    });

    c.bench_function("bench_iterate_next_graphemes_biredictional", |b| {
        b.iter(|| {
            let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 0));
            for grapheme in iter.by_ref() {
                black_box(grapheme);
            }
        })
    });

    c.bench_function("bench_iterate_prev_graphemes_biredictional", |b| {
        b.iter(|| {
            let mut iter = buffer.bidirectional_grapheme_iter(Position::new(0, 3 * 128));
            while let Some(grapheme) = iter.prev() {
                black_box(grapheme);
            }
        })
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
