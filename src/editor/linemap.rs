use std::{collections::BTreeMap, path::Path};

use bitflags::bitflags;
use noa_buffer::cursor::Position;

use crate::git::{DiffType, Repo};

bitflags! {
    pub struct LineStatus: u16 {
        const NONE     = 0b0000_0000_0000_0000;

        // Git diff results:
        const REPO_DIFF_MASK = 0b0000_0000_0000_0011;
        const ADDED          = 0b0000_0000_0000_0001;
        const REMOVED        = 0b0000_0000_0000_0010;
        const MODIFIED       = 0b0000_0000_0000_0011;

        // Cursors other than the main one:
        const MULTI_CURSOR_MASK = 0b0000_0000_0000_0100;
        const MULTI_CURSOR      = 0b0000_0000_0000_0100;
    }
}

pub struct LineMap {
    /// Line status for each physical line.
    lines: BTreeMap<usize /* y */, LineStatus>,
}

impl LineMap {
    pub fn new() -> LineMap {
        LineMap {
            lines: BTreeMap::new(),
        }
    }

    pub fn get(&self, y: usize) -> Option<LineStatus> {
        self.lines.get(&y).copied()
    }

    pub fn prev_diff_line(&self, y: usize) -> Option<Position> {
        let mut iter = self.lines.range(..y);
        let mut prev_y = y;
        while let Some((y, status)) = iter.next_back() {
            if (*status & LineStatus::REPO_DIFF_MASK).is_empty() {
                continue;
            }

            if *y == prev_y || *y == prev_y - 1 {
                prev_y = *y;
                continue;
            }

            return Some(Position::new(*y, 0));
        }

        None
    }

    pub fn next_diff_line(&self, y: usize) -> Option<Position> {
        let iter = self.lines.range(y + 1..);
        let mut prev_y = y;
        for (y, status) in iter {
            if (*status & LineStatus::REPO_DIFF_MASK).is_empty() {
                continue;
            }

            if *y == prev_y || *y == prev_y + 1 {
                prev_y = *y;
                continue;
            }

            return Some(Position::new(*y, 0));
        }

        None
    }

    pub fn insert_with_mask(&mut self, y: usize, status: LineStatus, clear_mask: LineStatus) {
        self.lines
            .entry(y)
            .and_modify(|v| {
                *v = status | (*v & !clear_mask);
            })
            .or_insert(status);
    }

    pub fn update_git_line_statuses(&mut self, repo: &Repo, buffer_path: &Path, text: &str) {
        let diffs = match repo.compute_line_diffs(buffer_path, text) {
            Ok(diffs) => diffs,
            Err(err) => {
                trace!("failed to get git diff: {:?}", err);
                return;
            }
        };

        for diff in diffs {
            let value = match diff.diff_type {
                DiffType::Added => LineStatus::ADDED,
                DiffType::Removed => LineStatus::REMOVED,
                DiffType::Modified => LineStatus::MODIFIED,
            };

            for y in diff.range {
                self.insert_with_mask(y, value, LineStatus::REPO_DIFF_MASK);
            }
        }
    }
}
