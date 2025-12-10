use crate::droid::{self, Options};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Output from the droid tool
#[derive(Debug, Serialize)]
struct DroidOutput {
    success: bool,
    #[serde(rename = "SESSION_ID")]
    session_id: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    warnings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_info: Option<String>,
}

/// Input parameters for droid tool
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DroidArgs {
    /// Instruction for task to send to droid (mutually exclusive with file)
    #[serde(rename = "PROMPT", default)]
    pub prompt: Option<String>,

    /// Read prompt from file (mutually exclusive with PROMPT)
    #[serde(default)]
    pub file: Option<PathBuf>,

    /// Autonomy level: low, medium, high (omit for DEFAULT/read-only)
    /// Cannot be used with skip_permissions_unsafe
    #[serde(default)]
    pub auto: Option<String>,

    /// Resume a previously started Droid session
    #[serde(rename = "SESSION_ID", default)]
    pub session_id: Option<String>,

    /// Working directory for execution (default: current directory)
    #[serde(default)]
    pub cwd: Option<PathBuf>,

    /// Model to use (overrides default)
    #[serde(default)]
    pub model: Option<String>,

    /// Enable specific tools (comma or space separated)
    #[serde(default)]
    pub enabled_tools: Option<String>,

    /// Disable specific tools (comma or space separated)
    #[serde(default)]
    pub disabled_tools: Option<String>,

    /// Timeout in seconds (default: 600, max: 3600)
    #[serde(default)]
    pub timeout_secs: Option<u64>,

    /// Reasoning effort level for supported models (low, medium, high)
    /// Maps to -r/--reasoning-effort flag
    #[serde(default)]
    pub reasoning_effort: Option<String>,

    /// Use specification mode (agent plans before executing)
    /// Maps to --use-spec flag
    #[serde(default)]
    pub use_spec: Option<bool>,

    /// Model to use for specification phase (when use_spec is true)
    /// Maps to --spec-model flag
    #[serde(default)]
    pub spec_model: Option<String>,

    /// Skip ALL permission checks (DANGEROUS - only for isolated environments)
    /// Cannot be combined with auto parameter
    /// Maps to --skip-permissions-unsafe flag
    #[serde(default)]
    pub skip_permissions_unsafe: Option<bool>,

    /// Output format: stream-json (default) or stream-jsonrpc
    #[serde(default)]
    pub output_format: Option<String>,
}

#[derive(Clone)]
pub struct DroidServer {
    tool_router: ToolRouter<DroidServer>,
}

impl Default for DroidServer {
    fn default() -> Self {
        Self::new()
    }
}

impl DroidServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl DroidServer {
    /// Executes a non-interactive Droid session via CLI to perform AI-assisted coding tasks
    ///
    /// **Return structure:**
    /// - `success`: boolean indicating execution status
    /// - `SESSION_ID`: unique identifier for resuming this conversation in future calls
    /// - `message`: concatenated assistant response text
    /// - `error`: error description when `success=False`
    /// - `warnings`: optional warnings (e.g., DROID.md truncation)
    ///
    /// **Best practices:**
    /// - Always capture and reuse `SESSION_ID` for multi-turn interactions
    /// - Use `auto` parameter to control operation permissions
    /// - Place a `DROID.md` file in working directory for project-specific context
    #[tool(
        name = "droid",
        description = "Execute Droid CLI for AI-assisted coding tasks with configurable autonomy levels"
    )]
    async fn droid(
        &self,
        Parameters(args): Parameters<DroidArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Validate prompt/file mutual exclusivity
        match (&args.prompt, &args.file) {
            (None, None) => {
                return Err(McpError::invalid_params(
                    "Either PROMPT or file parameter is required",
                    None,
                ));
            }
            (Some(p), None) if p.trim().is_empty() => {
                return Err(McpError::invalid_params(
                    "PROMPT must be a non-empty, non-whitespace string",
                    None,
                ));
            }
            (Some(_), Some(_)) => {
                return Err(McpError::invalid_params(
                    "PROMPT and file are mutually exclusive, provide only one",
                    None,
                ));
            }
            _ => {}
        }

        // Validate auto and skip_permissions_unsafe mutual exclusivity
        let skip_perms = args.skip_permissions_unsafe.unwrap_or(false);
        if skip_perms && args.auto.is_some() {
            return Err(McpError::invalid_params(
                "skip_permissions_unsafe cannot be combined with auto parameter",
                None,
            ));
        }

        // Resolve working directory
        let working_dir = if let Some(cwd) = args.cwd {
            let resolved = if cwd.is_absolute() {
                cwd
            } else {
                std::env::current_dir()
                    .map_err(|e| {
                        McpError::invalid_params(
                            format!("Failed to resolve current directory: {}", e),
                            None,
                        )
                    })?
                    .join(cwd)
            };
            resolved.canonicalize().map_err(|e| {
                McpError::invalid_params(
                    format!(
                        "Working directory does not exist or is not accessible: {} ({})",
                        resolved.display(),
                        e
                    ),
                    None,
                )
            })?
        } else {
            std::env::current_dir().map_err(|e| {
                McpError::invalid_params(
                    format!("Failed to resolve current working directory: {}", e),
                    None,
                )
            })?
        };

        if !working_dir.is_dir() {
            return Err(McpError::invalid_params(
                format!(
                    "Working directory is not a directory: {}",
                    working_dir.display()
                ),
                None,
            ));
        }

        // Validate file path if provided
        let file_path = if let Some(file) = args.file {
            let resolved = if file.is_absolute() {
                file
            } else {
                working_dir.join(file)
            };
            let canonical = resolved.canonicalize().map_err(|e| {
                McpError::invalid_params(
                    format!(
                        "File does not exist or is not accessible: {} ({})",
                        resolved.display(),
                        e
                    ),
                    None,
                )
            })?;
            if !canonical.is_file() {
                return Err(McpError::invalid_params(
                    format!("File path is not a file: {}", resolved.display()),
                    None,
                ));
            }
            Some(canonical)
        } else {
            None
        };

        // Filter empty strings to None
        let session_id = args.session_id.filter(|s| !s.is_empty());
        let auto = args.auto.filter(|s| !s.is_empty());
        let model = args.model.filter(|s| !s.is_empty());
        let enabled_tools = args.enabled_tools.filter(|s| !s.is_empty());
        let disabled_tools = args.disabled_tools.filter(|s| !s.is_empty());
        let reasoning_effort = args.reasoning_effort.filter(|s| !s.is_empty());
        let spec_model = args.spec_model.filter(|s| !s.is_empty());
        let output_format = args.output_format.filter(|s| !s.is_empty());

        // Validate autonomy level
        if let Some(ref level) = auto {
            match level.as_str() {
                "low" | "medium" | "high" => {}
                _ => {
                    return Err(McpError::invalid_params(
                        format!(
                            "Invalid auto level: '{}'. Must be one of: low, medium, high",
                            level
                        ),
                        None,
                    ));
                }
            }
        }

        // Validate reasoning effort level
        if let Some(ref level) = reasoning_effort {
            match level.as_str() {
                "low" | "medium" | "high" => {}
                _ => {
                    return Err(McpError::invalid_params(
                        format!(
                            "Invalid reasoning_effort: '{}'. Must be one of: low, medium, high",
                            level
                        ),
                        None,
                    ));
                }
            }
        }

        // Validate output format
        if let Some(ref format) = output_format {
            match format.as_str() {
                "stream-json" | "stream-jsonrpc" => {}
                _ => {
                    return Err(McpError::invalid_params(
                        format!(
                            "Invalid output_format: '{}'. Must be one of: stream-json, stream-jsonrpc",
                            format
                        ),
                        None,
                    ));
                }
            }
        }

        // Build Options
        let opts = Options {
            prompt: args.prompt,
            file: file_path,
            working_dir,
            session_id,
            auto,
            model,
            enabled_tools,
            disabled_tools,
            additional_args: droid::default_additional_args(),
            timeout_secs: args.timeout_secs,
            reasoning_effort,
            use_spec: args.use_spec.unwrap_or(false),
            spec_model,
            skip_permissions_unsafe: skip_perms,
            output_format,
        };

        // Execute droid
        let result = droid::run(opts).await.map_err(|e| {
            eprintln!("droid-mcp-rs: droid::run failed: {e:?}");
            McpError::internal_error(format!("Failed to execute droid: {e:?}"), None)
        })?;

        // Build output using TOON encoding
        let output = DroidOutput {
            success: result.success,
            session_id: result.session_id.clone(),
            message: result.agent_messages.clone(),
            error: result.error.clone(),
            warnings: result.warnings.clone(),
            model_info: result.model_info.clone(),
        };

        let toon_output = toon_format::encode_default(&output).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize output: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(toon_output)]))
    }
}

#[tool_handler]
impl ServerHandler for DroidServer {
    fn get_info(&self) -> ServerInfo {
        let custom_models = droid::list_custom_models();
        let models_info = if custom_models.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nAvailable custom models from ~/.factory/config.json:\n{}",
                custom_models
                    .iter()
                    .map(|m| format!("  - {}", m))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(format!(
                "This server provides a droid tool for AI-assisted coding tasks. \
                 Use the droid tool to execute coding tasks via the Droid CLI with \
                 configurable autonomy levels. Set autonomy level via the 'auto' parameter \
                 (low, medium, high) to control operation permissions. Place a DROID.md file \
                 in the working directory for project-specific context.{}",
                models_info
            )),
        }
    }
}
