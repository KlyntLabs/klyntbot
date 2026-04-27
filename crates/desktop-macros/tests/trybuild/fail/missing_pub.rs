//! Missing `pub` should fail.

#[desktop_macros::klynt_command]
async fn ping() -> i32 {
    42
}

fn main() {}
