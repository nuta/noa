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
/*
alpha
charlie
beta
*/
}

pub fn compute_git_diff(
    repo: &Repository,
    buffer: &Buffer,
) -> Result<Vec<LineStatus>, Box<dyn std::error::Error>> {
    let head_tree = repo.head()?.peel_to_tree()?;
    let new = buffer.text();
    let diff = repo.diff_tree_to_workdir(Some(&head_tree), None)?;

    let mut statuses = Vec::new();
    let mut start_y = None;
    let mut num_added = 0;
    let mut num_deleted = 0;
    let mut num_added_total = 0;
    let mut num_deleted_total = 0;
    diff.print(DiffFormat::Patch, |_, _, line| {
        trace!("-----------------------------------------");
        trace!("n={}, {:?} -> {:?}", line.num_lines(), line.old_lineno(),line.new_lineno());
        trace!("'{}': {}", line.origin(), std::str::from_utf8(line.content()).unwrap());
        match line.origin() {
            '+' => {
                if start_y.is_none() {
                    start_y = Some(line.new_lineno().unwrap() as usize - 1);
                }
                num_added += 1;
                num_added_total += 1;
            }
            '-' => {
                if start_y.is_none() {
                    start_y = Some(
                        line.old_lineno().unwrap() as usize - 1
                            + num_added_total - num_deleted_total
                    );
                }
                num_deleted += 1;
                num_deleted_total += 1;
            }
            ' ' => {
                if start_y.is_some() {
                    info!("y={:?}, +{} -{}", start_y, num_added, num_deleted);
                }
                start_y = None;
                num_added = 0;
                num_deleted = 0;
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
