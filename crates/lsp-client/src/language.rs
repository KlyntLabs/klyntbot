//! Map file extensions to LSP language server identifiers.

use std::path::Path;

/// Return the language server binary name for the given file path, based on extension.
pub fn language_for(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "rs" => Some("rust-analyzer"),
        "ts" | "tsx" | "js" | "jsx" => Some("typescript-language-server"),
        "py" => Some("pyright"),
        "go" => Some("gopls"),
        "c" | "h" => Some("clangd"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("clangd"),
        "java" => Some("jdtls"),
        "rb" => Some("solargraph"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_files_map_to_rust_analyzer() {
        assert_eq!(
            language_for(Path::new("src/main.rs")),
            Some("rust-analyzer")
        );
    }

    #[test]
    fn typescript_maps_to_tsserver() {
        assert_eq!(
            language_for(Path::new("index.ts")),
            Some("typescript-language-server")
        );
        assert_eq!(
            language_for(Path::new("App.tsx")),
            Some("typescript-language-server")
        );
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert_eq!(language_for(Path::new("README.md")), None);
        assert_eq!(language_for(Path::new("data.csv")), None);
    }

    #[test]
    fn no_extension_returns_none() {
        assert_eq!(language_for(Path::new("Makefile")), None);
    }
}
