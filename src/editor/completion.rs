use noa_common::fuzzyvec::FuzzyVec;

pub trait CompletionProvider {}

#[derive(Clone, Copy)]
pub enum CompletionKind {
    Word,
}

#[derive(Clone)]
pub struct CompletionItem {
    kind: CompletionKind,
    insert_text: String,
}

pub struct Completion {
    items: FuzzyVec<CompletionItem>,
}
