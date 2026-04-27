//! Missing `async` should fail.

#[desktop_macros::klynt_command]
pub fn ping() -> i32 {
    42
}

fn main() {}
