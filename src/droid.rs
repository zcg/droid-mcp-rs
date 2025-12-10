use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

// Constants
const DEFAULT_TIMEOUT_SECS: u64 = 600; // 10 minutes
const MAX_TIMEOUT_SECS: u64 = 3600; // 1 hour
const MAX_AGENT_MESSAGES_SIZE: usize = 10 * 1024 * 1024; // 10MB
const MAX_ALL_MESSAGES_SIZE: usize = 50 * 1024 * 1024; // 50MB
const MAX_DROID_MD_SIZE: usize = 1024 * 1024; // 1MB
const ABSOLUTE_MAX_SIZE: u64 = 10 * 1024 * 1024; // 10MB absolute max
const MAX_STDERR_SIZE: usize = 100_000; // 100KB

/// Droid CLI execution options
#[derive(Debug, Clone)]
pub struct Options {
    pub prompt: Option<String>,
    pub file: Option<PathBuf>,
    pub working_dir: PathBuf,
    pub session_id: Option<String>,
    pub auto: Option<String>,
    pub model: Option<String>,
    pub enabled_tools: Option<String>,
    pub disabled_tools: Option<String>,
    pub additional_args: Vec<String>,
    pub timeout_secs: Option<u64>,
    pub reasoning_effort: Option<String>,
    pub use_spec: bool,
    pub spec_model: Option<String>,
    pub skip_permissions_unsafe: bool,
    pub output_format: Option<String>,
}

/// Droid execution result
#[derive(Debug)]
pub struct DroidResult {
    pub success: bool,
    pub session_id: String,
    pub agent_messages: String,
    pub agent_messages_truncated: bool,
    pub all_messages: Vec<HashMap<String, Value>>,
    pub all_messages_truncated: bool,
    pub error: Option<String>,
    pub warnings: Option<String>,
    pub model_info: Option<String>,
}

/// Custom model configuration from Factory config
#[derive(Debug, Clone, Deserialize)]
struct CustomModel {
    model_display_name: String,
    model: String,
    #[serde(default)]
    provider: String,
}

/// Factory configuration file structure
#[derive(Debug, Clone, Deserialize)]
struct FactoryConfig {
    #[serde(default)]
    custom_models: Vec<CustomModel>,
}

/// Server configuration loaded from droid-mcp.config.json
#[derive(Debug, Clone, Deserialize)]
struct ServerConfig {
    #[serde(default)]
    additional_args: Vec<String>,
    timeout_secs: Option<u64>,
    default_auto: Option<String>,
    max_timeout_secs: Option<u64>,
    #[serde(default)]
    allow_high_autonomy: bool,
}

fn resolve_config_path() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("DROID_MCP_CONFIG_PATH") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join("droid-mcp.config.json"))
}

fn load_server_config() -> ServerConfig {
    let mut cfg = ServerConfig {
        additional_args: Vec::new(),
        timeout_secs: None,
        default_auto: None,
        max_timeout_secs: None,
        allow_high_autonomy: true,  // Default to true for high autonomy
    };

    let Some(config_path) = resolve_config_path() else {
        return cfg;
    };

    if !config_path.is_file() {
        return cfg;
    }

    match std::fs::read_to_string(&config_path) {
        Ok(raw) => match serde_json::from_str::<ServerConfig>(&raw) {
            Ok(parsed) => {
                let mut cleaned = parsed;
                cleaned.additional_args = cleaned
                    .additional_args
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                cfg = cleaned;
            }
            Err(err) => {
                eprintln!(
                    "droid-mcp-rs: failed to parse config {}: {}",
                    config_path.display(),
                    err
                );
            }
        },
        Err(err) => {
            eprintln!(
                "droid-mcp-rs: failed to read config {}: {}",
                config_path.display(),
                err
            );
        }
    }

    cfg
}

fn server_config() -> &'static ServerConfig {
    static SERVER_CONFIG: OnceLock<ServerConfig> = OnceLock::new();
    SERVER_CONFIG.get_or_init(load_server_config)
}

pub fn default_additional_args() -> Vec<String> {
    server_config().additional_args.clone()
}

pub fn default_timeout_secs() -> u64 {
    static CACHED_TIMEOUT: OnceLock<u64> = OnceLock::new();
    *CACHED_TIMEOUT.get_or_init(|| {
        let cfg = server_config();
        match cfg.timeout_secs {
            Some(t) if t > 0 && t <= MAX_TIMEOUT_SECS => t,
            Some(t) if t > MAX_TIMEOUT_SECS => MAX_TIMEOUT_SECS,
            _ => DEFAULT_TIMEOUT_SECS,
        }
    })
}

fn resolve_factory_config_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            return Some(PathBuf::from(user_profile).join(".factory").join("config.json"));
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return Some(PathBuf::from(home).join(".factory").join("config.json"));
        }
    }

    None
}

fn load_factory_config() -> FactoryConfig {
    let mut cfg = FactoryConfig {
        custom_models: Vec::new(),
    };

    let Some(config_path) = resolve_factory_config_path() else {
        return cfg;
    };

    if !config_path.is_file() {
        return cfg;
    }

    match std::fs::read_to_string(&config_path) {
        Ok(raw) => match serde_json::from_str::<FactoryConfig>(&raw) {
            Ok(parsed) => {
                cfg = parsed;
            }
            Err(err) => {
                eprintln!(
                    "droid-mcp-rs: failed to parse Factory config {}: {}",
                    config_path.display(),
                    err
                );
            }
        },
        Err(err) => {
            eprintln!(
                "droid-mcp-rs: failed to read Factory config {}: {}",
                config_path.display(),
                err
            );
        }
    }

    cfg
}

fn factory_config() -> &'static FactoryConfig {
    static FACTORY_CONFIG: OnceLock<FactoryConfig> = OnceLock::new();
    FACTORY_CONFIG.get_or_init(load_factory_config)
}

/// List all available custom models from Factory config
pub fn list_custom_models() -> Vec<String> {
    let cfg = factory_config();
    cfg.custom_models
        .iter()
        .enumerate()
        .map(|(idx, model)| {
            format!(
                "{} (custom:{}-{})",
                model.model_display_name,
                model.model_display_name.replace(' ', "-"),
                idx
            )
        })
        .collect()
}

/// Get the default autonomy level to use
fn get_default_auto() -> Option<String> {
    let cfg = server_config();
    if let Some(ref default_auto) = cfg.default_auto {
        if !default_auto.trim().is_empty() {
            return Some(default_auto.clone());
        }
    }
    // Default to high if not configured
    Some("high".to_string())
}

/// Get the default model to use (prefer GPT models, fallback to first custom model)
fn get_default_model() -> Option<String> {
    let cfg = factory_config();

    // Priority 1: Find first GPT model
    for (idx, model) in cfg.custom_models.iter().enumerate() {
        let name_lower = model.model_display_name.to_lowercase();
        let model_lower = model.model.to_lowercase();

        if name_lower.contains("gpt") || model_lower.contains("gpt") {
            let model_ref = format!(
                "custom:{}-{}",
                model.model_display_name.replace(' ', "-"),
                idx
            );
            return Some(model_ref);
        }
    }

    // Priority 2: Fallback to first custom model
    if let Some(first_model) = cfg.custom_models.first() {
        let model_ref = format!(
            "custom:{}-0",
            first_model.model_display_name.replace(' ', "-")
        );
        return Some(model_ref);
    }

    // Priority 3: No custom models - use Factory default
    None
}

/// Get model display name and details for logging and display
/// Returns: (model_info for result field, warning for user display)
fn get_model_info(model_param: &Option<String>) -> (Option<String>, Option<String>) {
    let cfg = factory_config();

    let Some(model) = model_param else {
        // This shouldn't happen since we apply defaults, but handle it anyway
        if let Some(first_model) = cfg.custom_models.first() {
            let display = format!(
                "{} [{}] ({})",
                first_model.model_display_name, first_model.provider, first_model.model
            );
            // No warning - silent default
            return (Some(display), None);
        } else {
            return (
                Some("Factory default model".to_string()),
                Some("⚠️  No custom models configured. Using Factory default model (requires FACTORY_API_KEY).".to_string()),
            );
        }
    };

    // Check if it's a custom model reference
    if model.starts_with("custom:") {
        // Parse custom model reference: "custom:Display-Name-0"
        if let Some((_, rest)) = model.split_once(':') {
            // Extract index from the end
            if let Some((_display_part, idx_str)) = rest.rsplit_once('-') {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if let Some(custom_model) = cfg.custom_models.get(idx) {
                        let display = format!(
                            "{} [{}] ({})",
                            custom_model.model_display_name,
                            custom_model.provider,
                            custom_model.model
                        );
                        // No warning - silent custom model
                        return (Some(display), None);
                    }
                }
            }
        }
        // Failed to parse custom model reference
        let warning = format!("⚠️  Invalid custom model reference: '{}'. Using first available model.", model);
        if let Some(first_model) = cfg.custom_models.first() {
            let display = format!(
                "{} [{}] ({})",
                first_model.model_display_name, first_model.provider, first_model.model
            );
            let combined_warning = format!("{}\nℹ️  Fallback to: {}", warning, display);
            return (Some(display), Some(combined_warning));
        } else {
            return (Some("Factory default model".to_string()), Some(warning));
        }
    }

    // Not a custom model - direct model string
    let display = format!("Model: {}", model);
    // No warning for explicit model parameter
    (Some(display), None)
}

/// Resolves the droid binary path
/// droid is typically in PATH (installed in ~/bin or C:\Users\<user>\bin)
/// Can be overridden with DROID_BIN environment variable
#[cfg(windows)]
fn resolve_droid_bin() -> String {
    if let Ok(val) = std::env::var("DROID_BIN") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    // droid is in PATH, default installation: C:\Users\<user>\bin\droid.exe
    "droid.exe".to_string()
}

#[cfg(not(windows))]
fn resolve_droid_bin() -> String {
    if let Ok(val) = std::env::var("DROID_BIN") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    // droid is in PATH, default installation: ~/bin/droid (no extension)
    "droid".to_string()
}

async fn read_droid_md(working_dir: &std::path::Path) -> (Option<String>, Option<String>) {
    let droid_path = working_dir.join("DROID.md");

    if !droid_path.exists() {
        return (None, None);
    }

    let metadata = match tokio::fs::metadata(&droid_path).await {
        Ok(m) => m,
        Err(e) => {
            let warning = format!("Failed to read DROID.md metadata: {}", e);
            return (None, Some(warning));
        }
    };

    let file_size = metadata.len();

    if file_size > ABSOLUTE_MAX_SIZE {
        let warning = format!(
            "DROID.md is {} bytes, exceeding the absolute maximum of {} bytes and will be skipped.",
            file_size, ABSOLUTE_MAX_SIZE
        );
        return (None, Some(warning));
    }

    let bytes_to_read = (file_size as usize).min(MAX_DROID_MD_SIZE + 4);
    let file = match tokio::fs::File::open(&droid_path).await {
        Ok(f) => f,
        Err(e) => {
            let warning = format!("Failed to open DROID.md: {}", e);
            return (None, Some(warning));
        }
    };

    let mut content = Vec::with_capacity(bytes_to_read);
    if let Err(e) = file
        .take(bytes_to_read as u64)
        .read_to_end(&mut content)
        .await
    {
        let warning = format!("Failed to read DROID.md: {}", e);
        return (None, Some(warning));
    }

    if content.is_empty() {
        return (None, None);
    }

    if file_size <= bytes_to_read as u64 {
        if let Ok(s) = std::str::from_utf8(&content) {
            if s.trim().is_empty() {
                return (None, None);
            }
        }
    }

    if content.len() > MAX_DROID_MD_SIZE {
        let mut end = MAX_DROID_MD_SIZE;
        while end > 0 {
            if let Ok(valid_str) = std::str::from_utf8(&content[..end]) {
                let warning = format!(
                    "DROID.md is {} bytes, exceeding the {} byte limit and was truncated to {} bytes.",
                    file_size, MAX_DROID_MD_SIZE, end
                );
                return (Some(valid_str.to_string()), Some(warning));
            }
            end -= 1;
        }
        let warning = "DROID.md contains invalid UTF-8 and was skipped.".to_string();
        (None, Some(warning))
    } else {
        match String::from_utf8(content) {
            Ok(s) => (Some(s), None),
            Err(_) => {
                let warning = "DROID.md contains invalid UTF-8 and was skipped.".to_string();
                (None, Some(warning))
            }
        }
    }
}

pub async fn run(mut opts: Options) -> Result<DroidResult> {
    // Apply default model if not specified
    if opts.model.is_none() {
        opts.model = get_default_model();
    }

    // Apply default autonomy level if not specified
    if opts.auto.is_none() {
        opts.auto = get_default_auto();
    }

    let (droid_content, droid_warning) = read_droid_md(&opts.working_dir).await;
    let mut prompt_to_use = String::new();

    if let Some(content) = droid_content {
        prompt_to_use.push_str("<system_prompt>\n");
        prompt_to_use.push_str(&content);
        prompt_to_use.push_str("\n</system_prompt>\n\n");
    }

    if let Some(ref prompt) = opts.prompt {
        prompt_to_use.push_str(prompt);
    }

    if opts.timeout_secs.is_none() {
        opts.timeout_secs = Some(default_timeout_secs());
    }

    let timeout_secs = opts.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
    let cfg = server_config();
    let max_timeout = cfg.max_timeout_secs.unwrap_or(MAX_TIMEOUT_SECS);
    let timeout_secs = timeout_secs.min(max_timeout);
    let duration = std::time::Duration::from_secs(timeout_secs);

    if let Some(ref auto) = opts.auto {
        if auto == "high" && !cfg.allow_high_autonomy {
            return Err(anyhow::anyhow!(
                "High autonomy level is disabled in configuration. Set allow_high_autonomy=true to enable."
            ));
        }
    }

    match tokio::time::timeout(
        duration,
        run_internal(opts.clone(), prompt_to_use, droid_warning.clone()),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            let (model_display, model_warning) = get_model_info(&opts.model);
            let timeout_warning = format!("Droid execution timed out after {} seconds", timeout_secs);
            let combined_warning = match (droid_warning, model_warning) {
                (Some(dw), Some(mw)) => Some(format!("{}\n{}\n{}", dw, mw, timeout_warning)),
                (Some(dw), None) => Some(format!("{}\n{}", dw, timeout_warning)),
                (None, Some(mw)) => Some(format!("{}\n{}", mw, timeout_warning)),
                (None, None) => Some(timeout_warning),
            };
            let result = DroidResult {
                success: false,
                session_id: String::new(),
                agent_messages: String::new(),
                agent_messages_truncated: false,
                all_messages: Vec::new(),
                all_messages_truncated: false,
                error: Some(format!("Timeout after {} seconds", timeout_secs)),
                warnings: combined_warning,
                model_info: model_display,
            };
            Ok(result)
        }
    }
}

async fn run_internal(
    opts: Options,
    prompt: String,
    mut droid_warning: Option<String>,
) -> Result<DroidResult> {
    let droid_bin = resolve_droid_bin();

    // Get model info for logging and display
    let (model_display, model_warning) = get_model_info(&opts.model);

    // Log to stderr for debugging
    if let Some(ref info) = model_display {
        eprintln!("droid-mcp-rs: {}", info);
    }

    // Merge model warning into droid_warning
    if let Some(model_warn) = model_warning {
        droid_warning = Some(match droid_warning {
            Some(existing) => format!("{}\n{}", existing, model_warn),
            None => model_warn,
        });
    }

    let mut cmd = Command::new(&droid_bin);
    cmd.args(["exec"]);

    // Output format (default to stream-json if not specified)
    let output_fmt = opts.output_format.as_deref().unwrap_or("stream-json");
    cmd.arg("-o");
    cmd.arg(output_fmt);

    cmd.arg("--cwd");
    cmd.arg(opts.working_dir.as_os_str());

    // Skip permissions unsafe (mutually exclusive with auto)
    if opts.skip_permissions_unsafe {
        cmd.arg("--skip-permissions-unsafe");
    } else if let Some(ref auto) = opts.auto {
        cmd.arg("--auto");
        cmd.arg(auto);
    }

    // Reasoning effort
    if let Some(ref reasoning) = opts.reasoning_effort {
        cmd.arg("-r");
        cmd.arg(reasoning);
    }

    // Specification mode
    if opts.use_spec {
        cmd.arg("--use-spec");
        if let Some(ref spec_model) = opts.spec_model {
            cmd.arg("--spec-model");
            cmd.arg(spec_model);
        }
    }

    if let Some(ref model) = opts.model {
        cmd.arg("--model");
        cmd.arg(model);
    }

    if let Some(ref enabled) = opts.enabled_tools {
        cmd.arg("--enabled-tools");
        cmd.arg(enabled);
    }

    if let Some(ref disabled) = opts.disabled_tools {
        cmd.arg("--disabled-tools");
        cmd.arg(disabled);
    }

    if let Some(ref session_id) = opts.session_id {
        cmd.arg("--session-id");
        cmd.arg(session_id);
    }

    for arg in &opts.additional_args {
        cmd.arg(arg);
    }

    if let Some(ref file) = opts.file {
        cmd.arg("--file");
        cmd.arg(file);
    } else {
        cmd.arg(prompt);
    }

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn().with_context(|| {
        format!(
            "Failed to spawn droid command '{}' in '{}'",
            droid_bin,
            opts.working_dir.display()
        )
    })?;

    let stdout = child
        .stdout
        .take()
        .context("Failed to get stdout from droid command")?;
    let stderr = child
        .stderr
        .take()
        .context("Failed to get stderr from droid command")?;

    let mut result = DroidResult {
        success: true,
        session_id: String::new(),
        agent_messages: String::new(),
        agent_messages_truncated: false,
        all_messages: Vec::new(),
        all_messages_truncated: false,
        error: None,
        warnings: droid_warning,
        model_info: model_display,
    };

    let stderr_handle = tokio::spawn(async move {
        let mut stderr_output = String::new();
        let mut stderr_reader = BufReader::new(stderr);
        let mut line = String::new();

        loop {
            line.clear();
            match stderr_reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    if stderr_output.len() + line.len() <= MAX_STDERR_SIZE {
                        stderr_output.push_str(&line);
                    }
                }
                Err(_) => break,
            }
        }
        stderr_output
    });

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let mut all_messages_size: usize = 0;

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let line_data: Value = match serde_json::from_str(trimmed) {
                    Ok(data) => data,
                    Err(e) => {
                        eprintln!("droid-mcp-rs: failed to parse JSON line: {}", e);
                        continue;
                    }
                };

                if let Some(sid) = line_data.get("session_id").and_then(|v| v.as_str()) {
                    if !sid.is_empty() && result.session_id.is_empty() {
                        result.session_id = sid.to_string();
                    }
                }

                if let Some(line_type) = line_data.get("type").and_then(|v| v.as_str()) {
                    if line_type == "error" {
                        result.success = false;
                        if let Some(msg) = line_data.get("message").and_then(|v| v.as_str()) {
                            result.error = Some(format!("droid error: {}", msg));
                        }
                    }

                    // Extract completion finalText (this is the final response from droid)
                    if line_type == "completion" {
                        if let Some(final_text) =
                            line_data.get("finalText").and_then(|v| v.as_str())
                        {
                            let new_size = result.agent_messages.len() + final_text.len();
                            if new_size > MAX_AGENT_MESSAGES_SIZE {
                                if !result.agent_messages_truncated {
                                    result.agent_messages.push_str(
                                        "\n[... Agent messages truncated due to size limit ...]",
                                    );
                                    result.agent_messages_truncated = true;
                                }
                            } else if !result.agent_messages_truncated {
                                if !result.agent_messages.is_empty() && !final_text.is_empty() {
                                    result.agent_messages.push('\n');
                                }
                                result.agent_messages.push_str(final_text);
                            }
                        }
                    }

                    // Also extract intermediate assistant messages for context
                    if line_type == "message" {
                        if let Some(role) = line_data.get("role").and_then(|v| v.as_str()) {
                            if role == "assistant" {
                                // Droid uses "text" field for intermediate messages
                                if let Some(text) =
                                    line_data.get("text").and_then(|v| v.as_str())
                                {
                                    let new_size = result.agent_messages.len() + text.len();
                                    if new_size > MAX_AGENT_MESSAGES_SIZE {
                                        if !result.agent_messages_truncated {
                                            result.agent_messages.push_str(
                                                "\n[... Agent messages truncated due to size limit ...]",
                                            );
                                            result.agent_messages_truncated = true;
                                        }
                                    } else if !result.agent_messages_truncated {
                                        if !result.agent_messages.is_empty() && !text.is_empty()
                                        {
                                            result.agent_messages.push('\n');
                                        }
                                        result.agent_messages.push_str(text);
                                    }
                                }
                            }
                        }
                    }
                }

                if let Ok(map) =
                    serde_json::from_value::<HashMap<String, Value>>(line_data.clone())
                {
                    let message_size = serde_json::to_string(&map).map(|s| s.len()).unwrap_or(0);
                    if all_messages_size + message_size <= MAX_ALL_MESSAGES_SIZE {
                        all_messages_size += message_size;
                        result.all_messages.push(map);
                    } else if !result.all_messages_truncated {
                        result.all_messages_truncated = true;
                    }
                }
            }
            Err(e) => {
                eprintln!("droid-mcp-rs: failed to read line: {}", e);
                break;
            }
        }
    }

    let status = child
        .wait()
        .await
        .context("Failed to wait for droid command")?;

    let stderr_output = match stderr_handle.await {
        Ok(output) => output,
        Err(e) => {
            eprintln!("droid-mcp-rs: failed to join stderr task: {}", e);
            String::new()
        }
    };

    if !status.success() {
        result.success = false;
        if result.error.is_none() {
            let error_msg = if !stderr_output.is_empty() {
                format!(
                    "droid exited with code {:?}. stderr: {}",
                    status.code(),
                    stderr_output
                )
            } else {
                format!("droid exited with code {:?}", status.code())
            };
            result.error = Some(error_msg);
        }
    }

    if result.session_id.is_empty() {
        result.success = false;
        result.error = Some("No session_id received from droid".to_string());
    }

    if result.agent_messages.is_empty() && result.success {
        result.success = false;
        result.error = Some("No agent messages received from droid".to_string());
    }

    Ok(result)
}
