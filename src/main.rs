mod accessibility;
mod browser;
mod cdp;
mod error;
mod mcp;
mod native_input;
mod screenshot;

use clap::Parser;
use mcp::McpServer;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Chrome browser automation via MCP – click anywhere
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Chrome DevTools host
    #[arg(long, default_value = "localhost")]
    chrome_host: String,

    /// Chrome DevTools port
    #[arg(long, default_value_t = 9222)]
    chrome_port: u16,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Run server over stdio (MCP protocol)
    #[arg(long, default_value_t = true)]
    stdio: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing
    let log_level = match args.log_level.to_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr) // Write logs to stderr to avoid interfering with stdio MCP protocol
                .with_ansi(false) // Disable ANSI colors for cleaner logs
                .with_target(false)
                .with_thread_ids(false)
                .compact(),
        )
        .with(tracing_subscriber::filter::LevelFilter::from_level(log_level))
        .init();

    info!("Starting chrome-mcp server");
    info!("Chrome host: {}", args.chrome_host);
    info!("Chrome port: {}", args.chrome_port);
    info!("Log level: {}", args.log_level);

    // Create MCP server
    let mut server = match McpServer::new(&args.chrome_host, args.chrome_port) {
        Ok(server) => server,
        Err(e) => {
            error!("Failed to create MCP server: {}", e);
            return Err(e.into());
        }
    };

    // Check if Chrome is accessible
    info!("Checking Chrome connection...");
    // We'll handle connection errors gracefully in the initialize handler

    if args.stdio {
        info!("Running MCP server over stdio");
        if let Err(e) = server.run_stdio().await {
            error!("MCP server error: {}", e);
            return Err(e.into());
        }
    } else {
        error!("Only stdio mode is currently supported");
        return Err("Only stdio mode is currently supported".into());
    }

    info!("chrome-mcp server shutting down");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing() {
        use clap::Parser;
        
        let args = Args::parse_from(&[
            "chrome-mcp",
            "--chrome-host", "127.0.0.1",
            "--chrome-port", "9223",
            "--log-level", "debug"
        ]);

        assert_eq!(args.chrome_host, "127.0.0.1");
        assert_eq!(args.chrome_port, 9223);
        assert_eq!(args.log_level, "debug");
        assert!(args.stdio);
    }

    #[test]
    fn test_default_args() {
        use clap::Parser;
        
        let args = Args::parse_from(&["chrome-mcp"]);

        assert_eq!(args.chrome_host, "localhost");
        assert_eq!(args.chrome_port, 9222);
        assert_eq!(args.log_level, "info");
        assert!(args.stdio);
    }
}