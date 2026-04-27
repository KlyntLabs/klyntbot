//! Applying to a struct should fail.

#[desktop_macros::klynt_command]
pub struct Foo {
    bar: i32,
}

fn main() {}
