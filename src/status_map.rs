use crate::buffer::Buffer;
use git2::{DiffFormat, Repository};
use std::ops::RangeInclusive;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineStatusType {
    Added,
    Modified,
    Deleted,
    Error,
    Warning,
}

#[derive(Clone, Debug)]
pub struct LineStatus {
    pub lines: RangeInclusive<usize>,
    pub status: LineStatusType,
    pub message: Option<String>,
}

impl LineStatus {
    pub fn new(
        status: LineStatusType,
        lines: RangeInclusive<usize>,
        message: Option<String>,
    ) -> LineStatus {
        LineStatus {
            status,
            lines,
            message,
        }
    }
}

pub struct StatusMap {
    statuses: Vec<LineStatus>,
}

impl StatusMap {
    pub fn new() -> StatusMap {
        StatusMap {
            statuses: Vec::new(),
        }
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&LineStatus) -> bool,
    {
        self.statuses.retain(f);
    }

    pub fn add(
        &mut self,
        status: LineStatusType,
        lines: RangeInclusive<usize>,
        message: Option<String>,
    ) {
        self.statuses.push(LineStatus::new(status, lines, message));
    }

    pub fn get(&self, y: usize) -> Option<&LineStatus> {
        for ls in self.statuses.iter().rev() {
            if ls.lines.contains(&y) {
                return Some(ls);
            }
        }

        None
    }

    pub fn get_by_range(&self, y: usize, size: usize) -> Option<&LineStatus> {
        for ls in self.statuses.iter().rev() {
            let start = *ls.lines.start();
            let end = *ls.lines.end();
            if y <= end && start <= y + size {
                return Some(ls);
            }
        }

        None
    }
}

fn is_same_file(path1: &Path, path2: &Path) -> bool {
    use std::fs::metadata;
    use std::os::unix::fs::MetadataExt;
    match (metadata(path1), metadata(path2)) {
        (Ok(meta1), Ok(meta2)) => meta1.ino() == meta2.ino(),
        _ => false,
    }
}

pub fn compute_git_diff(
    statuses: &mut StatusMap,
    repo: &Repository,
    buffer: &Buffer,
) -> Result<(), Box<dyn std::error::Error>> {
    let head_tree = repo.head()?.peel_to_tree()?;
    let diff = repo.diff_tree_to_workdir(Some(&head_tree), None)?;

    let mut start_y = None;
    let mut num_added = 0;
    let mut num_deleted = 0;
    let mut num_added_total = 0;
    let mut num_deleted_total = 0;
    diff.print(DiffFormat::Patch, |delta, _, line| {
        match (buffer.path(), delta.new_file().path()) {
            (Some(path1), Some(path2)) if is_same_file(path1, path2) => {
                // This diff is for `buffer`. Continue processing.
            }
            _ => return true,
        }

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
                        line.old_lineno().unwrap() as usize - 1 + num_added_total
                            - num_deleted_total,
                    );
                }
                num_deleted += 1;
                num_deleted_total += 1;
            }
            'F' => {
                num_added_total = 0;
                num_deleted_total = 0;
            }
            _ => {
                match (start_y, num_added > 0, num_deleted > 0) {
                    // Added.
                    (Some(start), true, false) => {
                        let lines = start..=(start + num_added - 1);
                        statuses.add(LineStatusType::Added, lines, None);
                    }
                    // Deleted.
                    (Some(start), false, true) => {
                        let lines = start..=start;
                        statuses.add(LineStatusType::Deleted, lines, None);
                    }
                    // Modified.
                    (Some(start), true, true) => {
                        let lines = start..=(start + num_added - 1);
                        statuses.add(LineStatusType::Modified, lines, None);
                    }
                    _ => {}
                }
                start_y = None;
                num_added = 0;
                num_deleted = 0;
            }
        }

        match (start_y, num_added > 0, num_deleted > 0) {
            // Added.
            (Some(start), true, false) => {
                let lines = start..=(start + num_added - 1);
                statuses.add(LineStatusType::Added, lines, None);
            }
            // Deleted.
            (Some(start), false, true) => {
                let lines = start..=start;
                statuses.add(LineStatusType::Deleted, lines, None);
            }
            // Modified.
            (Some(start), true, true) => {
                let lines = start..=(start + num_added - 1);
                statuses.add(LineStatusType::Modified, lines, None);
            }
            _ => {}
        }

        // Continue the iteration.
        true
    })
    .ok();

    Ok(())
}
