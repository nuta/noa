#![feature(test)]
extern crate test;

#[macro_use]
extern crate log;

pub mod buffer;
pub mod char_iter;
pub mod cursor;
pub mod display_width;
pub mod extras;
pub mod find;
pub mod grapheme_iter;
pub mod mut_raw_buffer;
pub mod paragraph_iter;
pub mod raw_buffer;
pub mod reflow_iter;
pub mod syntax;
pub mod word_iter;
