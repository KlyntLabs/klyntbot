//! `klyntbot plugin new` — scaffold a new plugin project.

use anyhow::{bail, Result};
use std::path::PathBuf;

/// Scaffold a new plugin project directory.
pub async fn run(name: &str, lang: &str) -> Result<()> {
    match lang {
        "rust" => scaffold_rust(name).await,
        "typescript" | "ts" => scaffold_typescript(name).await,
        "python" | "py" => scaffold_python(name).await,
        _ => bail!("Unsupported language '{lang}'. Choose: rust, typescript, python"),
    }
}

async fn scaffold_rust(name: &str) -> Result<()> {
    let dir = PathBuf::from(name);
    if dir.exists() {
        bail!("Directory '{name}' already exists.");
    }

    let src_dir = dir.join("src");
    tokio::fs::create_dir_all(&src_dir).await?;

    // Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
extism-pdk = "1"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#
    );
    tokio::fs::write(dir.join("Cargo.toml"), cargo_toml).await?;

    // src/lib.rs
    let lib_rs = format!(
        r#"use extism_pdk::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct HelloInput {{
    name: String,
}}

#[plugin_fn]
pub fn {fn_name}(Json(input): Json<HelloInput>) -> FnResult<String> {{
    Ok(format!("Hello from {name}: {{}}", input.name))
}}
"#,
        fn_name = name.replace('-', "_"),
    );
    tokio::fs::write(src_dir.join("lib.rs"), lib_rs).await?;

    // Manifest
    let manifest = serde_json::json!({
        "id": name,
        "name": name,
        "version": "0.1.0",
        "description": format!("A klyntbot plugin: {name}"),
        "author": "you",
        "tools": [{
            "name": name.replace('-', "_"),
            "description": format!("Run the {name} plugin"),
            "parameters": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name to greet" }
                },
                "required": ["name"]
            }
        }]
    });
    tokio::fs::write(
        dir.join("klyntbot.plugin.json"),
        serde_json::to_string_pretty(&manifest)?,
    )
    .await?;

    println!("Created Rust plugin project: {name}/");
    println!("  {name}/Cargo.toml");
    println!("  {name}/src/lib.rs");
    println!("  {name}/klyntbot.plugin.json");
    println!("\nBuild with: cargo build --target wasm32-wasip1 --release");
    println!("Install with: klyntbot plugin install ./{name}/");
    Ok(())
}

async fn scaffold_typescript(name: &str) -> Result<()> {
    let dir = PathBuf::from(name);
    if dir.exists() {
        bail!("Directory '{name}' already exists.");
    }

    let src_dir = dir.join("src");
    tokio::fs::create_dir_all(&src_dir).await?;

    // package.json
    let pkg = serde_json::json!({
        "name": name,
        "version": "0.1.0",
        "scripts": {
            "build": "extism-js src/index.ts -o plugin.wasm"
        },
        "devDependencies": {
            "@anthropic/extism-js": "latest"
        }
    });
    tokio::fs::write(
        dir.join("package.json"),
        serde_json::to_string_pretty(&pkg)?,
    )
    .await?;

    // src/index.ts
    let index_ts = format!(
        r#"import {{ Host }} from "@anthropic/extism-js";

export function {fn_name}() {{
    const input = JSON.parse(Host.inputString());
    Host.outputString(`Hello from {name}: ${{input.name}}`);
}}
"#,
        fn_name = name.replace('-', "_"),
    );
    tokio::fs::write(src_dir.join("index.ts"), index_ts).await?;

    // Manifest
    let manifest = serde_json::json!({
        "id": name,
        "name": name,
        "version": "0.1.0",
        "description": format!("A klyntbot plugin: {name}"),
        "author": "you",
        "tools": [{
            "name": name.replace('-', "_"),
            "description": format!("Run the {name} plugin"),
            "parameters": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name to greet" }
                },
                "required": ["name"]
            }
        }]
    });
    tokio::fs::write(
        dir.join("klyntbot.plugin.json"),
        serde_json::to_string_pretty(&manifest)?,
    )
    .await?;

    println!("Created TypeScript plugin project: {name}/");
    println!("  {name}/package.json");
    println!("  {name}/src/index.ts");
    println!("  {name}/klyntbot.plugin.json");
    println!("\nBuild with: npm run build");
    println!("Install with: klyntbot plugin install ./{name}/");
    Ok(())
}

async fn scaffold_python(name: &str) -> Result<()> {
    let dir = PathBuf::from(name);
    if dir.exists() {
        bail!("Directory '{name}' already exists.");
    }

    let src_dir = dir.join("src");
    tokio::fs::create_dir_all(&src_dir).await?;

    // src/main.py
    let main_py = format!(
        r#"import extism
import json

@extism.plugin_fn
def {fn_name}():
    input = json.loads(extism.input_str())
    extism.output_str(f"Hello from {name}: {{input['name']}}")
"#,
        fn_name = name.replace('-', "_"),
    );
    tokio::fs::write(src_dir.join("main.py"), main_py).await?;

    // Manifest
    let manifest = serde_json::json!({
        "id": name,
        "name": name,
        "version": "0.1.0",
        "description": format!("A klyntbot plugin: {name}"),
        "author": "you",
        "tools": [{
            "name": name.replace('-', "_"),
            "description": format!("Run the {name} plugin"),
            "parameters": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name to greet" }
                },
                "required": ["name"]
            }
        }]
    });
    tokio::fs::write(
        dir.join("klyntbot.plugin.json"),
        serde_json::to_string_pretty(&manifest)?,
    )
    .await?;

    println!("Created Python plugin project: {name}/");
    println!("  {name}/src/main.py");
    println!("  {name}/klyntbot.plugin.json");
    println!("\nBuild with: extism-py src/main.py -o plugin.wasm");
    println!("Install with: klyntbot plugin install ./{name}/");
    Ok(())
}
