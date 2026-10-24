#![allow(unsafe_code)]

// This module is the sole libc identity boundary; callers only receive the
// effective UID and never interact with a raw pointer or foreign handle.
pub fn effective_user_id() -> libc::uid_t {
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    unsafe { libc::geteuid() }
}
