//! Returning `Result<T, E>` should fail.

#[desktop_macros::klynt_command]
pub async fn ping() -> Result<i32, String> {
    Ok(42)
}

fn main() {}
