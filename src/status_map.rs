use git2::{Blob, Diff, DiffOptions, Error, Object, ObjectType, Oid, Repository};
use git2::{DiffDelta, DiffFindOptions, DiffFormat, DiffHunk, DiffLine};
use std::ops::RangeInclusive;
use crate::buffer::Buffer;

pub enum LineStatusType {
}

pub struct LineStatus {
    lines: RangeInclusive<usize>,
    status: LineStatusType,
}

pub fn compute_git_diff(
    repo: &Repository,
    buffer: &Buffer,
) -> Result<Vec<LineStatus>, Box<dyn std::error::Error>> {
    let head_tree = repo.head()?.peel_to_tree()?;
    let new = buffer.text();
    let diff = repo.diff_tree_to_workdir(Some(&head_tree), None)?;

    let statuses = Vec::new();
    diff.print(DiffFormat::Patch, |delta, hunk, line| {
        trace!("----------------------------------------");
        trace!("delta={:?}", delta);
        trace!("hunk={:?}", hunk);
        trace!("line={:?}", line);

        // Continue the iteration.
        true
    });
    Ok(statuses)
}
