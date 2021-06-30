use std::ops::Range;

use anyhow::Result;
use git2::{Diff, DiffOptions, Repository};

pub enum DiffType {
    Added,
    Modified,
    Removed,
}

pub struct LineRangeDiff {
    diff_type: DiffType,
    range: Range<usize>,
}

pub fn compute_line_diff_status() -> Result<()> {
    let repo = Repository::init("/home/seiya/noa")?;
    let diff = repo.diff_tree_to_workdir(None, None)?;
    println!("computing git diff...");
    diff.foreach(
        &mut |_, _| true,
        None,
        Some(&mut |delta, hunk| {
            println!("hunk = {:#?}", hunk);
            true
        }),
        Some(&mut |delta, hunk, line| {
            println!("line = {:#?}", line);
            true
        }),
    )
    .unwrap();
    Ok(())
}
