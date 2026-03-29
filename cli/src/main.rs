use clap::{Parser, Subcommand};
use std::process;

mod client;
mod commands;
mod cdp;

#[derive(Parser)]
#[command(name = "browser-cli", about = "Browserless Chrome CLI — zero raw API calls")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Request timeout in seconds
    #[arg(long, global = true, default_value = "30")]
    timeout: u64,
}

#[derive(Subcommand)]
enum Commands {
    /// Take a screenshot of a URL or render local HTML to PNG
    Screenshot {
        /// URL to screenshot (omit if using --html)
        url: Option<String>,

        /// Local HTML file to render to PNG
        #[arg(long)]
        html: Option<String>,

        /// Output file path
        #[arg(short, long, default_value = "screenshot.png")]
        output: String,

        /// Viewport width
        #[arg(long, default_value = "1400")]
        width: u32,

        /// Viewport height
        #[arg(long, default_value = "900")]
        height: u32,

        /// Capture full page
        #[arg(long)]
        full_page: bool,

        /// Wait for CSS selector before capture
        #[arg(long)]
        wait_for: Option<String>,

        /// Delay in ms after page load before capture
        #[arg(long)]
        delay: Option<u64>,
    },

    /// Extract page content
    Content {
        /// URL to extract content from
        url: String,

        /// Output format: html, text, markdown
        #[arg(long, default_value = "markdown")]
        format: String,
    },

    /// Generate PDF from URL
    Pdf {
        /// URL to render
        url: String,

        /// Output file path
        #[arg(short, long, default_value = "output.pdf")]
        output: String,

        /// Landscape orientation
        #[arg(long)]
        landscape: bool,

        /// Paper format
        #[arg(long, default_value = "A4")]
        format: String,
    },

    /// Scrape elements from a page
    Scrape {
        /// URL to scrape
        url: String,

        /// CSS selector
        #[arg(short, long)]
        elements: String,

        /// Properties to extract (comma-separated)
        #[arg(short, long, default_value = "textContent")]
        attributes: String,
    },

    /// Check browserless health/pressure
    Health,

    /// HTTP fetch through browserless (residential IP proxy)
    Fetch {
        /// URL to fetch
        url: String,

        /// HTTP method
        #[arg(short, long, default_value = "GET")]
        method: String,

        /// Headers (key:value format, repeatable)
        #[arg(short = 'H', long)]
        headers: Vec<String>,

        /// Request body
        #[arg(short, long)]
        body: Option<String>,

        /// Cookie session name for multi-step flows
        #[arg(long)]
        cookie_session: Option<String>,
    },

    /// Execute CDP automation script via WebSocket
    Cdp {
        /// Path to JSON-lines script file
        script: String,

        /// Timeout in ms
        #[arg(long, default_value = "30000")]
        cdp_timeout: u64,
    },

    /// Simple GET proxy through browserless
    Proxy {
        /// URL to fetch
        url: String,
    },
}

#[tokio::main]
async fn main() {
    // Install rustls CryptoProvider for CDP WebSocket TLS
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cli = Cli::parse();

    let config = match client::Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Configuration error: {e}");
            process::exit(1);
        }
    };

    let result = match cli.command {
        Commands::Screenshot { url, html, output, width, height, full_page, wait_for, delay } => {
            commands::screenshot(&config, url, html, &output, width, height, full_page, wait_for, delay, cli.timeout, cli.json).await
        }
        Commands::Content { url, format } => {
            commands::content(&config, &url, &format, cli.timeout, cli.json).await
        }
        Commands::Pdf { url, output, landscape, format } => {
            commands::pdf(&config, &url, &output, landscape, &format, cli.timeout, cli.json).await
        }
        Commands::Scrape { url, elements, attributes } => {
            commands::scrape(&config, &url, &elements, &attributes, cli.timeout, cli.json).await
        }
        Commands::Health => {
            commands::health(&config, cli.timeout, cli.json).await
        }
        Commands::Fetch { url, method, headers, body, cookie_session } => {
            commands::fetch(&config, &url, &method, &headers, body.as_deref(), cookie_session.as_deref(), cli.timeout, cli.json).await
        }
        Commands::Cdp { script, cdp_timeout } => {
            cdp::run_script(&config, &script, cdp_timeout).await
        }
        Commands::Proxy { url } => {
            commands::proxy(&config, &url, cli.timeout, cli.json).await
        }
    };

    if let Err(e) = result {
        eprintln!("❌ {e}");
        process::exit(1);
    }
}
