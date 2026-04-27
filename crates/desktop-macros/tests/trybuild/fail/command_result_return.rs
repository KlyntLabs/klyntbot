//! Returning `CommandResult<T>` should fail.

#[desktop_macros::klynt_command]
pub async fn ping() -> desktop_shared::CommandResult<i32> {
    Ok(42)
}

fn main() {}
