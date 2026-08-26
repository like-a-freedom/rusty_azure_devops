use rmcp::serve_server;
use rusty_azure_devops::{AdoMcpHandler, config::Config, create_modules};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure logging: prefer ADO-specific env, fallback to RUST_LOG
    let log_env = std::env::var("ADO_MCP_LOG_LEVEL")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "info".to_string());
    let combined_filter = format!(
        "{},rmcp=warn,serve_inner=warn,azure_devops_rust_api=info",
        log_env
    );
    let env_filter = tracing_subscriber::EnvFilter::try_new(combined_filter).unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new(
            "info,rmcp=warn,serve_inner=warn,azure_devops_rust_api=info",
        )
    });

    tracing_subscriber::fmt()
        .compact()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .with_env_filter(env_filter)
        .init();

    tracing::info!("rusty_azure_devops: starting stdio MCP server...");

    // Load configuration from environment; require ADO_TOKEN and ADO_BASE_URL
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error=%e, "missing required environment variables; ADO_TOKEN and ADO_BASE_URL must be set");
            eprintln!(
                "Error: {}. Please set ADO_TOKEN and ADO_BASE_URL environment variables and try again.",
                e
            );
            std::process::exit(1);
        }
    };

    let modules = create_modules(&config).map_err(|e| {
        tracing::error!(error=%e, "failed to create modules");
        e
    })?;
    let handler = AdoMcpHandler::new(
        modules.work_items,
        modules.links,
        modules.comments,
        modules.projects,
        modules.queries,
    );

    tracing::info!("rusty_azure_devops: ready to serve (tools will be published to clients)");

    // Start RMCP server over stdio transport so it's immediately usable with MCP clients
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let server = serve_server(handler, transport).await?;

    tracing::info!("rusty_azure_devops: module set initialised as server");

    server.waiting().await?;

    Ok(())
}
