use anyhow::Result;
use clap::Parser;
use droid_mcp_rs::server::DroidServer;
use rmcp::{transport::stdio, ServiceExt};

/// MCP server wrapping the Droid CLI for AI-assisted coding tasks
#[derive(Parser)]
#[command(
    name = "droid-mcp-rs",
    version,
    about = "MCP server that provides AI-assisted coding through the Droid CLI",
    long_about = None,
    after_help = "ENVIRONMENT VARIABLES:
  DROID_BIN                    Override the droid binary path
                               Default: 'droid' (Linux/macOS) or 'droid.exe' (Windows)
                               Typical installation: ~/bin/droid or C:\\Users\\<user>\\bin\\droid.exe
  DROID_MCP_CONFIG_PATH        Path to configuration file (default: './droid-mcp.config.json')

USAGE:
  This server communicates via stdio using the Model Context Protocol (MCP).
  It should be configured in your MCP client (e.g., Claude Desktop) settings.

  Example MCP client configuration:
    {
      \"mcpServers\": {
        \"droid\": {
          \"command\": \"/path/to/droid-mcp-rs\"
        }
      }
    }

SUPPORTED PARAMETERS:
  The 'droid' tool accepts the following parameters:

  PROMPT (string)              Task instruction to send to Droid (mutually exclusive with file)
  file (path)                  Read prompt from file (mutually exclusive with PROMPT)
  auto (string)                Autonomy level: low, medium, high (omit for DEFAULT/read-only)
  SESSION_ID (string)          Resume an existing session (from previous response)
  cwd (path)                   Working directory for the Droid session (default: current directory)
  model (string)               Model to use (overrides default)
  enabled_tools (string)       Comma/space-separated list of tools to enable
  disabled_tools (string)      Comma/space-separated list of tools to disable
  timeout_secs (number)        Timeout in seconds (default: 600, max: 3600)

DROID.MD SUPPORT:
  If a DROID.md file exists in the working directory, its content will be
  automatically prepended to the prompt as a system prompt. This allows you to
  define project-specific instructions or context for all Droid invocations.

  Maximum file size: 1 MB (larger files will be truncated)

AUTONOMY LEVELS:
  DEFAULT (no --auto flag)     Read-only operations (cat, git status, ls)
  low                          File creation/editing in project directories
  medium                       Package installation, git commits, local builds
  high                         Git push, production deployments, script execution

  Note: 'high' autonomy is disabled by default. Set allow_high_autonomy=true
  in droid-mcp.config.json to enable it.

SECURITY:
  - By default, only read-only operations are allowed (DEFAULT autonomy)
  - Set autonomy level explicitly to enable modifications
  - Timeouts are enforced to prevent unbounded execution
  - High autonomy level requires explicit configuration
  - See autonomy levels above for security controls

CONFIGURATION FILE (droid-mcp.config.json):
  {
    \"additional_args\": [],
    \"timeout_secs\": 600,
    \"max_timeout_secs\": 3600,
    \"default_auto\": \"low\",
    \"default_model\": \"claude-opus-4-5-20251101\",
    \"allow_high_autonomy\": false
  }

For more information, visit: https://github.com/jakvbs/droid-mcp-rs"
)]
struct Cli {}

#[tokio::main]
async fn main() -> Result<()> {
    let _cli = Cli::parse();

    let service = DroidServer::new().serve(stdio()).await.inspect_err(|e| {
        eprintln!("serving error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}
