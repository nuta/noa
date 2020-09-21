use git2::{Blob, Diff, DiffOptions, Error, Object, ObjectType, Oid, Repository};
use git2::{DiffDelta, DiffFindOptions, DiffFormat, DiffHunk, DiffLine};
use std::collections::HashSet;
use std::ops::RangeInclusive;
use crate::buffer::Buffer;

pub enum LineStatusType {
    Modified,
    Deleted,
}

pub struct LineStatus {
    lines: RangeInclusive<usize>,
    status: LineStatusType,
}

impl LineStatus {
    pub fn new(
        status: LineStatusType,
        lines: RangeInclusive<usize>,
    ) -> LineStatus {
        LineStatus {
            status,
            lines,
        }
    }
}

pub fn compute_git_diff(
    repo: &Repository,
    buffer: &Buffer,
) -> Result<Vec<LineStatus>, Box<dyn std::error::Error>> {
    let head_tree = repo.head()?.peel_to_tree()?;
    let new = buffer.text();
    let diff = repo.diff_tree_to_workdir(Some(&head_tree), None)?;

    let mut statuses = Vec::new();
    let mut deleted_lines = HashSet::new();
    let mut inserted_lines = HashSet::new();
    diff.print(DiffFormat::Patch, |delta, hunk, line| {
        trace!("----------------------------------------");
        trace!("{}: {:?} -> {:?}", line.num_lines(), line.old_lineno(),line.new_lineno());
        match (line.old_lineno(), line.new_lineno()) {
            (None, Some(lineno)) => {
                let y = lineno as usize - 1;
                statuses.push(LineStatus::new(LineStatusType::Modified, y..=y));
            }
            (Some(lineno), None) => {
                let y = lineno as usize - 1;
                statuses.push(LineStatus::new(LineStatusType::Deleted, y..=y));
            }
            _ => {}
        }

        // Continue the iteration.
        true
    });
    Ok(statuses)
}
