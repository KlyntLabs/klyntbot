fn main() {
    eprintln!("klyntbot v{}", env!("CARGO_PKG_VERSION"));
    eprintln!();
    eprintln!("The CLI has been removed. Use the desktop app instead:");
    eprintln!("  cargo tauri dev          # Development mode");
    eprintln!("  cargo tauri build        # Production build");
}
