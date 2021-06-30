use anyhow::Result;
use git2::{Diff, DiffOptions, Repository};

use std::ops;

pub enum DiffType {
    Added,
    Modified,
    Removed,
}

pub struct LineRangeDiff {
    diff_type: DiffType,
    range: ops::Range<usize>,
}

pub fn compute_line_diff_status() -> Result<()> {
    let repo = Repository::init("/Users/seiya/dev/noa")?;
    println!(">>> revparse");
    println!(
        "{:#?}",
        repo.revparse_single(&format!("HEAD:src/noa/git.rs"))?
            .peel_to_blob()
    );
    println!(">>> head");
    let blob = repo
        .revparse_single(&format!("HEAD:src/noa/git.rs"))?
        .peel_to_blob()?;
    println!(">>> diff");
    // let diff = repo.diff_tree_to_workdir(/*Some(&blob)*/ None, None)?;
    let diff = repo.diff_index_to_workdir(None, None)?;
    // repo.diff_blobs(
    //     Some(&blob),
    //     None,
    //     None,
    //     None,
    //     None,
    //     None,
    //     None,
    //     Some(&mut |delta, hunk| {
    //         println!(">>> hunk = {:#?}", hunk);
    //         true
    //     }),
    //     Some(&mut |delta, hunk, line| {
    //         // println!(">>> line = {:#?}", line);
    //         true
    //     }),
    // )?;

    println!(">>> computing git diff...");

    diff.foreach(
        &mut |_, _| true,
        None,
        Some(&mut |delta, hunk| {
            // println!(">>> delta = {:?}", hunk);
            println!(">>> hunk = {:?}", std::str::from_utf8(hunk.header()));
            true
        }),
        Some(&mut |delta, hunk, line| {
            // println!(">>> line = {:#?}", line);
            true
        }),
    )
    .unwrap();

    Ok(())
}
