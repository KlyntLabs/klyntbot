//! Missing return type should fail.

#[desktop_macros::klynt_command]
pub async fn ping() {
    ()
}

fn main() {}
