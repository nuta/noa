use git2::{Blob, Diff, DiffOptions, Error, Object, ObjectType, Oid, Repository};
use git2::{DiffDelta, DiffFindOptions, DiffFormat, DiffHunk, DiffLine};
use std::collections::HashSet;
use std::ops::RangeInclusive;
use crate::buffer::Buffer;

pub enum LineStatusType {
    Added,
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

fn add_diff_status(statuses: &mut Vec<LineStatus>, status: LineStatusType, y: usize) {

}

pub fn compute_git_diff(
    repo: &Repository,
    buffer: &Buffer,
) -> Result<Vec<LineStatus>, Box<dyn std::error::Error>> {
    let head_tree = repo.head()?.peel_to_tree()?;
    let new = buffer.text();
    let diff = repo.diff_tree_to_workdir(Some(&head_tree), None)?;

    let mut statuses = Vec::new();
    let mut next = true;
    let mut num_added = 0;
//    let mut changed_lines = HashSet::new();
    diff.print(DiffFormat::Patch, |_, _, line| {
        trace!("-----------------------------------------");
        trace!("n={}, {:?} -> {:?}", line.num_lines(), line.old_lineno(),line.new_lineno());
        trace!("origin: '{}'\ncontent:\n{}", line.origin(), std::str::from_utf8(line.content()).unwrap());
        match line.origin() {
            '+' => {
                // let old = line.old_lineno().unwrap() as usize - 1;
                // add_diff_status(&mut statuses, LineStatusType::Added, old);
            }
            ' ' => {
                next = true;
                num_added = 0;
            }
            _ => {
            }
        }
        match (line.old_lineno(), line.new_lineno()) {
            (None, Some(lineno)) => {
                let y = lineno as usize - 1;
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
