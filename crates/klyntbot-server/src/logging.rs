use tracing_subscriber::EnvFilter;

/// Configure tracing for stdio transport — all output to stderr.
pub fn configure_stdio_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}

/// Configure tracing for HTTP transport — standard output.
pub fn configure_http_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}
