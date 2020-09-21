use git2::{Blob, Diff, DiffOptions, Error, Object, ObjectType, Oid, Repository};
use git2::{DiffDelta, DiffFindOptions, DiffFormat, DiffHunk, DiffLine};
use std::collections::HashSet;
use std::ops::RangeInclusive;
use crate::buffer::Buffer;

#[derive(Clone, Copy, Debug)]
pub enum LineStatusType {
    Added,
    Modified,
    Deleted,
}

#[derive(Clone, Debug)]
pub struct LineStatus {
    pub lines: RangeInclusive<usize>,
    pub status: LineStatusType,
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
    let mut start_y = None;
    let mut num_added = 0;
    let mut num_deleted = 0;
    let mut num_added_total = 0;
    let mut num_deleted_total = 0;
    diff.print(DiffFormat::Patch, |_, _, line| {
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
                match (start_y, num_added > 0, num_deleted > 0) {
                    // Added.
                    (Some(start), true, false) => {
                        let lines = start..=(start + num_added - 1);
                        statuses.push(LineStatus::new(LineStatusType::Added, lines));
                    }
                    // Deleted.
                    (Some(start), false, true) => {
                        let lines = start..=start;
                        statuses.push(LineStatus::new(LineStatusType::Deleted, lines));
                    }
                    // Modified.
                    (Some(start), true, true) => {
                        let lines = start..=(start + num_added - 1);
                        statuses.push(LineStatus::new(LineStatusType::Modified, lines));
                    }
                    _ => {}
                }
                start_y = None;
                num_added = 0;
                num_deleted = 0;
            }
            _ => {
            }
        }

        // Continue the iteration.
        true
    });

    Ok(statuses)
}
