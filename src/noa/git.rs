use anyhow::Result;
use git2::DiffOptions;
use libgit2_sys::{
    git_blob, git_diff_blob_to_buffer, git_libgit2_init, git_object_peel, git_repository_open,
    git_revparse_single, GIT_OBJECT_BLOB,
};

use std::{
    ffi::{c_void, CString},
    ops,
    os::raw::c_char,
    ptr,
};

pub enum DiffType {
    Added,
    Modified,
    Removed,
}

pub struct LineRangeDiff {
    diff_type: DiffType,
    range: ops::Range<usize>,
}

struct DiffCallbackContext {}

macro_rules! try_libgit_func {
    ($summary:expr, $expr:expr) => {{
        let ret = $expr;
        if ret < 0 {
            let error = ::libgit2_sys::git_error_last();
            if error.is_null() {
                ::anyhow::bail!("libgit: failed to {}: returned {}", $summary, ret);
            }

            let message = ::std::ffi::CStr::from_ptr((*error).message as *const _)
                .to_str()
                .unwrap();

            ::anyhow::bail!(format!("libgit: failed to {}: {}", $summary, message));
        }
    }};
}

pub fn compute_line_diff_status() -> Result<()> {
    let buffer = include_str!("git.rs");
    unsafe {
        let mut opts = DiffOptions::new();
        let mut ctx = DiffCallbackContext {};

        let repo_path = CString::new("/Users/seiya/dev/noa").unwrap();
        let spec = CString::new(format!("HEAD:src/noa/git.rs")).unwrap();

        try_libgit_func!("init libgit2", git_libgit2_init());

        let mut repo = ptr::null_mut();
        try_libgit_func!(
            "open the repo",
            git_repository_open(&mut repo, repo_path.as_ptr())
        );

        let mut obj = ptr::null_mut();
        try_libgit_func!(
            "open the blob",
            git_revparse_single(&mut obj, repo, spec.as_ptr())
        );
        let mut blob = ptr::null_mut();
        try_libgit_func!(
            "peel the object",
            git_object_peel(&mut blob, obj, GIT_OBJECT_BLOB)
        );

        let old_as_path = std::ffi::CString::new("in_blob").unwrap();
        let buffer_as_path = std::ffi::CString::new("in_buf").unwrap();
        git_diff_blob_to_buffer(
            /* old_blob */ blob as *const git_blob,
            /* old_as_path */ old_as_path.as_ptr(),
            /* buffer */ buffer.as_bytes().as_ptr() as *const c_char,
            /* buffer_len */ buffer.as_bytes().len(),
            /* buffer_as_path */ buffer_as_path.as_ptr(),
            /* options */ opts.raw(),
            /* file_cb */ None,
            /* binary_cb */ None,
            /* hunk_cb */ None,
            /* line_cb */ None,
            /* payload */ &mut ctx as *mut _ as *mut c_void,
        );
    }

    // let diff = repo.diff_tree_to_workdir(/*Some(&blob)*/ None, None)?;
    // let diff = repo.diff_index_to_workdir(None, None)?;
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

    // println!(">>> computing git diff...");

    // diff.foreach(
    //     &mut |_, _| true,
    //     None,
    //     Some(&mut |delta, hunk| {
    //         // println!(">>> delta = {:?}", hunk);
    //         println!(">>> hunk = {:?}", std::str::from_utf8(hunk.header()));
    //         true
    //     }),
    //     Some(&mut |delta, hunk, line| {
    //         // println!(">>> line = {:#?}", line);
    //         true
    //     }),
    // )
    // .unwrap();

    Ok(())
}
