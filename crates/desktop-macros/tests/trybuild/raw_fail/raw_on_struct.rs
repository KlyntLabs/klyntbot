//! Applying `#[klynt_raw_command]` to a struct should fail.

#[desktop_macros::klynt_raw_command]
pub struct Foo {
    bar: i32,
}

fn main() {}
