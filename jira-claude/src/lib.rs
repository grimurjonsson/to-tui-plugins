//! Jira-Claude plugin for to-tui.
//!
//! Generates todos from Jira tickets using Claude AI.

#![allow(non_local_definitions)]

use abi_stable::{
    export_root_module,
    prefix_type::PrefixTypeTrait,
    sabi_trait::TD_Opaque,
    std_types::{RBox, RHashMap, ROption, RResult, RString, RVec},
};
use serde::Deserialize;
use std::fmt::Debug;
use std::process::Command;
use totui_plugin_interface::{
    FfiCommand, FfiConfigSchema, FfiConfigValue, FfiEvent, FfiEventType, FfiHookResponse,
    FfiTodoItem, FfiTodoState, HostApi_TO, Plugin, PluginModule, PluginModule_Ref, Plugin_TO,
};
use uuid::Uuid;

// ============================================================================
// Module export for abi_stable
// ============================================================================

#[export_root_module]
fn get_library() -> PluginModule_Ref {
    PluginModule { create_plugin }.leak_into_prefix()
}

extern "C" fn create_plugin() -> Plugin_TO<'static, RBox<()>> {
    Plugin_TO::from_value(JiraClaudePlugin::new(), TD_Opaque)
}

// ============================================================================
// Plugin implementation
// ============================================================================

/// The Jira-Claude plugin.
///
/// Fetches Jira ticket details via `acli` CLI and generates todos using Claude CLI.
#[derive(Debug)]
pub struct JiraClaudePlugin;

impl JiraClaudePlugin {
    pub fn new() -> Self {
        Self
    }

    /// Check if a command exists in PATH.
    fn check_command_exists(command: &str) -> Result<(), String> {
        let check = if cfg!(windows) {
            Command::new("where").arg(command).output()
        } else {
            Command::new("which").arg(command).output()
        };

        match check {
            Ok(output) if output.status.success() => Ok(()),
            _ => Err(format!("'{}' not found in PATH", command)),
        }
    }

    /// Run a command and return stdout.
    fn run_command(command: &str, args: &[&str]) -> Result<String, String> {
        let output = Command::new(command)
            .args(args)
            .output()
            .map_err(|e| format!("Failed to execute '{}': {}", command, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "'{}' failed with exit code {:?}: {}",
                command,
                output.status.code(),
                stderr.trim()
            ));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| format!("Invalid UTF-8 output from '{}': {}", command, e))
    }

    /// Fetch Jira ticket data via acli.
    fn fetch_jira_ticket(&self, ticket_id: &str) -> Result<JiraTicket, String> {
        let output = Self::run_command(
            "acli",
            &[
                "jira",
                "workitem",
                "view",
                ticket_id,
                "--fields",
                "key,summary,description,comment",
                "--json",
            ],
        )?;

        let ticket: AcliTicket = serde_json::from_str(&output).map_err(|e| {
            format!(
                "Failed to parse acli JSON output: {}\n\nRaw output:\n{}",
                e,
                truncate_string(&output, 2000)
            )
        })?;

        Ok(JiraTicket::from(ticket))
    }

    /// Fetch Jira base URL from acli auth status.
    fn fetch_jira_base_url(&self) -> Result<String, String> {
        let output = Self::run_command("acli", &["jira", "auth", "status"])?;

        for line in output.lines() {
            if let Some(site) = line.trim().strip_prefix("Site:") {
                let domain = site.trim();
                return Ok(format!("https://{}", domain));
            }
        }

        Err("Could not find Site in acli jira auth status output".to_string())
    }

    /// Generate todos using Claude CLI.
    fn generate_todos_with_claude(&self, ticket: &JiraTicket) -> Result<Vec<GeneratedTodo>, String> {
        let prompt = self.build_prompt(ticket);

        let output = Self::run_command("claude", &["-p", &prompt])?;

        self.parse_claude_output(&output)
    }

    /// Build the Claude prompt for todo generation.
    fn build_prompt(&self, ticket: &JiraTicket) -> String {
        let comments_section = if ticket.comments.is_empty() {
            String::from("No comments")
        } else {
            ticket
                .comments
                .iter()
                .map(|c| {
                    let author = c.author.as_deref().unwrap_or("Unknown");
                    let date = c
                        .created
                        .as_ref()
                        .and_then(|d| d.split('T').next())
                        .unwrap_or("Unknown date");
                    format!("- {} ({}): {}", author, date, c.body)
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            r#"You are a task breakdown assistant. Given a Jira ticket, generate actionable todo items.

TICKET: {}
SUMMARY: {}
DESCRIPTION:
{}

COMMENTS:
{}

Generate a list of specific, actionable todos to complete this ticket.
Each todo should be a concrete task that can be checked off.
Use nested todos for subtasks (indent_level > 0).
Consider any additional context or requirements mentioned in the comments.

IMPORTANT: Respond ONLY with a JSON array, no other text. Format:
[
  {{"content": "Main task description", "indent_level": 0}},
  {{"content": "Subtask description", "indent_level": 1}}
]"#,
            ticket.key,
            ticket.summary,
            ticket.description.as_deref().unwrap_or("No description"),
            comments_section
        )
    }

    /// Parse Claude's JSON output into generated todos.
    fn parse_claude_output(&self, output: &str) -> Result<Vec<GeneratedTodo>, String> {
        let trimmed = output.trim();

        let json_start = trimmed.find('[').ok_or_else(|| {
            format!(
                "Claude output doesn't contain JSON array. Output: {}",
                truncate_string(trimmed, 200)
            )
        })?;

        let json_end = trimmed.rfind(']').ok_or_else(|| {
            format!(
                "Claude output doesn't contain valid JSON array end. Output: {}",
                truncate_string(trimmed, 200)
            )
        })?;

        let json_str = &trimmed[json_start..=json_end];

        serde_json::from_str(json_str).map_err(|e| {
            format!(
                "Failed to parse Claude's JSON output: {}: {}",
                e,
                truncate_string(json_str, 200)
            )
        })
    }

    /// Generate todos from a Jira ticket ID.
    fn generate_from_ticket(&self, input: &str) -> Result<Vec<FfiTodoItem>, String> {
        // Validate required commands are available
        Self::check_command_exists("acli")?;
        Self::check_command_exists("claude")?;

        let ticket_id = input.trim().to_uppercase();

        let ticket = self
            .fetch_jira_ticket(&ticket_id)
            .map_err(|e| format!("Failed to fetch Jira ticket '{}': {}", ticket_id, e))?;

        let generated = self
            .generate_todos_with_claude(&ticket)
            .map_err(|e| format!("Failed to generate todos with Claude: {}", e))?;

        if generated.is_empty() {
            return Err(format!(
                "Claude generated no todos for ticket '{}'",
                ticket_id
            ));
        }

        let ticket_url = self
            .fetch_jira_base_url()
            .map(|base| format!("{}/browse/{}", base, ticket.key))
            .ok();

        let description = match (&ticket_url, &ticket.description) {
            (Some(url), Some(desc)) => Some(format!("{}\n---\n{}", url, desc)),
            (Some(url), None) => Some(url.clone()),
            (None, Some(desc)) => Some(desc.clone()),
            (None, None) => None,
        };

        // Create root todo with ticket summary
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let root_id = Uuid::new_v4().to_string();
        let root = FfiTodoItem {
            id: root_id.clone().into(),
            content: format!("{} : {}", ticket.key, ticket.summary).into(),
            state: FfiTodoState::Empty,
            priority: ROption::RNone,
            due_date: ROption::RNone,
            description: description.map(RString::from).into(),
            parent_id: ROption::RNone,
            indent_level: 0,
            created_at: now,
            modified_at: now,
            completed_at: ROption::RNone,
            position: 0,
        };

        // Create child todos from generated items
        let mut items = vec![root];
        for (idx, g) in generated.into_iter().enumerate() {
            let child = FfiTodoItem {
                id: Uuid::new_v4().to_string().into(),
                content: g.content.into(),
                state: FfiTodoState::Empty,
                priority: ROption::RNone,
                due_date: ROption::RNone,
                description: ROption::RNone,
                parent_id: ROption::RSome(root_id.clone().into()),
                indent_level: (g.indent_level + 1) as u32,
                created_at: now,
                modified_at: now,
                completed_at: ROption::RNone,
                position: (idx + 1) as u32,
            };
            items.push(child);
        }

        Ok(items)
    }
}

impl Default for JiraClaudePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for JiraClaudePlugin {
    fn name(&self) -> RString {
        "jira-claude".into()
    }

    fn version(&self) -> RString {
        "0.1.0".into()
    }

    fn min_interface_version(&self) -> RString {
        "0.1.0".into()
    }

    fn generate(&self, input: RString) -> RResult<RVec<FfiTodoItem>, RString> {
        match self.generate_from_ticket(input.as_str()) {
            Ok(items) => RResult::ROk(items.into_iter().collect()),
            Err(e) => RResult::RErr(e.into()),
        }
    }

    fn config_schema(&self) -> FfiConfigSchema {
        // No configuration needed for this plugin
        FfiConfigSchema::empty()
    }

    fn execute_with_host(
        &self,
        _input: RString,
        _host: HostApi_TO<'_, RBox<()>>,
    ) -> RResult<RVec<FfiCommand>, RString> {
        // This plugin uses generate() instead of execute_with_host()
        RResult::ROk(RVec::new())
    }

    fn on_config_loaded(&self, _config: RHashMap<RString, FfiConfigValue>) {
        // No config to process
    }

    fn subscribed_events(&self) -> RVec<FfiEventType> {
        // No event subscriptions
        RVec::new()
    }

    fn on_event(&self, _event: FfiEvent) -> RResult<FfiHookResponse, RString> {
        // No event handling
        RResult::ROk(FfiHookResponse::default())
    }
}

// ============================================================================
// Helper types for Jira data parsing
// ============================================================================

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[derive(Debug, Deserialize)]
struct AcliTicket {
    key: String,
    fields: JiraFields,
}

#[derive(Debug, Deserialize)]
struct JiraFields {
    summary: String,
    description: Option<serde_json::Value>,
    comment: Option<CommentWrapper>,
}

struct JiraTicket {
    key: String,
    summary: String,
    description: Option<String>,
    comments: Vec<JiraComment>,
}

impl From<AcliTicket> for JiraTicket {
    fn from(ticket: AcliTicket) -> Self {
        let description = ticket
            .fields
            .description
            .and_then(|v| extract_text_from_adf(&v));

        let comments = ticket
            .fields
            .comment
            .map(|wrapper| {
                wrapper
                    .comments
                    .into_iter()
                    .filter_map(|c| {
                        let body = c.body.and_then(|v| extract_text_from_adf(&v))?;
                        Some(JiraComment {
                            author: c.author.and_then(|a| a.display_name),
                            body,
                            created: c.created,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self {
            key: ticket.key,
            summary: ticket.fields.summary,
            description,
            comments,
        }
    }
}

fn extract_text_from_adf(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(obj) => {
            if let Some(content) = obj.get("content") {
                extract_text_from_adf(content)
            } else if let Some(text) = obj.get("text") {
                text.as_str().map(|s| s.to_string())
            } else {
                None
            }
        }
        serde_json::Value::Array(arr) => {
            let texts: Vec<String> = arr.iter().filter_map(extract_text_from_adf).collect();
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        }
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct GeneratedTodo {
    content: String,
    indent_level: usize,
}

#[derive(Debug, Deserialize)]
struct CommentWrapper {
    comments: Vec<AcliComment>,
}

#[derive(Debug, Deserialize)]
struct AcliComment {
    author: Option<CommentAuthor>,
    body: Option<serde_json::Value>,
    created: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentAuthor {
    display_name: Option<String>,
}

#[derive(Debug, Clone)]
struct JiraComment {
    author: Option<String>,
    body: String,
    created: Option<String>,
}
