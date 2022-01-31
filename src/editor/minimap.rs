use std::{collections::HashMap, path::Path};

use bitflags::bitflags;


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

pub struct MiniMap {
    /// Line status for each physical line.
    lines: HashMap<usize /* y */, LineStatus>,
}

impl MiniMap {
    pub fn new() -> MiniMap {
        MiniMap {
            lines: HashMap::new(),
        }
    }

    pub fn get(&self, y: usize) -> Option<LineStatus> {
        self.lines.get(&y).copied()
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
            trace!(
                "git diff: range={:?}, type={:?}",
                diff.range,
                diff.diff_type
            );

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