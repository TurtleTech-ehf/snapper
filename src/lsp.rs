use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::config::ProjectConfig;
use crate::format::Format;
use crate::{format_text, FormatConfig};

pub struct SnapperLsp {
    client: Client,
    documents: Mutex<HashMap<Url, (String, Format)>>,
    project_config: Mutex<ProjectConfig>,
    root_path: Mutex<Option<PathBuf>>,
}

impl SnapperLsp {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Mutex::new(HashMap::new()),
            project_config: Mutex::new(ProjectConfig::default()),
            root_path: Mutex::new(None),
        }
    }

    fn reload_config(&self) {
        let root = self.root_path.lock().expect("root_path lock poisoned");
        let config = if let Some(ref root) = *root {
            ProjectConfig::find_and_load(root).unwrap_or_default()
        } else {
            ProjectConfig::default()
        };
        *self.project_config.lock().expect("config lock poisoned") = config;
    }

    fn make_config(&self, format: Format) -> FormatConfig {
        let project = self.project_config.lock().expect("config lock poisoned");
        let format_str = match format {
            Format::Org => "org",
            Format::Latex => "latex",
            Format::Markdown => "markdown",
            Format::Rst => "rst",
            Format::Plaintext => "plaintext",
        };
        FormatConfig {
            format,
            max_width: project.max_width_for_format(format_str).unwrap_or(0),
            extra_abbreviations: project.abbreviations_for_format(format_str),
            ..Default::default()
        }
    }

    fn format_document(&self, uri: &Url) -> Option<Vec<TextEdit>> {
        // Returns None if lock is poisoned (graceful degradation)
        let docs = self.documents.lock().ok()?;
        let (text, format) = docs.get(uri)?;
        let config = self.make_config(*format);
        let formatted = format_text(text, &config).ok()?;

        if formatted == *text {
            return None;
        }

        let lines_count = text.lines().count();
        let end_line = lines_count.saturating_sub(1);
        let last_line_len = text.lines().last().map_or(0, |l| l.len());

        Some(vec![TextEdit {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(end_line as u32, last_line_len as u32),
            },
            new_text: formatted,
        }])
    }

    fn compute_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        let docs = self.documents.lock().expect("document store poisoned");
        let Some((text, _)) = docs.get(uri) else {
            return vec![];
        };

        let mut diagnostics = Vec::new();
        // Flag lines that contain multiple sentences (period + space + capital)
        for (i, line) in text.lines().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            if chars.is_empty() {
                continue;
            }

            // Highlight the exact space character where a split should occur
            for j in 1..chars.len().saturating_sub(1) {
                if (chars[j - 1] == '.' || chars[j - 1] == '!' || chars[j - 1] == '?')
                    && chars[j] == ' '
                    && chars.get(j + 1).is_some_and(|c| c.is_uppercase())
                {
                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position::new(i as u32, j as u32),
                            end: Position::new(i as u32, (j + 1) as u32),
                        },
                        severity: Some(DiagnosticSeverity::HINT),
                        source: Some("snapper".to_string()),
                        message: "Semantic line break recommended here. Consider running snapper."
                            .to_string(),
                        ..Default::default()
                    });
                }
            }
        }
        diagnostics
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SnapperLsp {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let root = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first())
            .and_then(|f| f.uri.to_file_path().ok())
            .or_else(|| {
                #[allow(deprecated)]
                params.root_uri.as_ref().and_then(|u| u.to_file_path().ok())
            });

        if let Some(ref root) = root {
            *self.root_path.lock().expect("root_path lock poisoned") = Some(root.clone());
        }
        self.reload_config();

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_range_formatting_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                document_on_type_formatting_provider: Some(DocumentOnTypeFormattingOptions {
                    first_trigger_character: ".".to_string(),
                    more_trigger_character: Some(vec![
                        " ".to_string(),
                        "?".to_string(),
                        "!".to_string(),
                        "\n".to_string(),
                    ]),
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["snapper.reloadConfig".to_string()],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let msg = {
            let config = self.project_config.lock().expect("config lock poisoned");
            format!(
                "snapper LSP initialized ({} extra abbreviations, max_width={})",
                config.extra_abbreviations.len(),
                config.max_width.unwrap_or(0),
            )
        };
        self.client.log_message(MessageType::INFO, msg).await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        let format = detect_format_from_uri(&uri, &params.text_document.language_id);

        self.documents
            .lock()
            .expect("document store poisoned")
            .insert(uri.clone(), (text, format));

        let diagnostics = self.compute_diagnostics(&uri);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(change) = params.content_changes.into_iter().last() {
            let format = {
                let docs = self.documents.lock().expect("document store poisoned");
                docs.get(&uri).map_or(Format::Plaintext, |(_, f)| *f)
            };
            self.documents
                .lock()
                .expect("document store poisoned")
                .insert(uri.clone(), (change.text, format));

            let diagnostics = self.compute_diagnostics(&uri);
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .lock()
            .expect("document store poisoned")
            .remove(&params.text_document.uri);
    }

    async fn did_change_watched_files(&self, _params: DidChangeWatchedFilesParams) {
        self.reload_config();
        self.client
            .log_message(MessageType::INFO, "Reloaded .snapperrc.toml")
            .await;

        let uris: Vec<Url> = {
            let docs = self.documents.lock().expect("document store poisoned");
            docs.keys().cloned().collect()
        };
        for uri in uris {
            let diagnostics = self.compute_diagnostics(&uri);
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        Ok(self.format_document(&params.text_document.uri))
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = &params.text_document.uri;
        let range = params.range;

        let docs = self.documents.lock().expect("document store poisoned");
        let Some((text, format)) = docs.get(uri) else {
            return Ok(None);
        };

        let lines: Vec<&str> = text.lines().collect();
        let start = range.start.line as usize;
        let end = (range.end.line as usize).min(lines.len().saturating_sub(1));
        let range_text = lines[start..=end].join("\n");

        let config = self.make_config(*format);
        let formatted = match format_text(&range_text, &config) {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };

        if formatted == range_text {
            return Ok(None);
        }

        let last_col = lines.get(end).map_or(0, |l| l.len());

        Ok(Some(vec![TextEdit {
            range: Range {
                start: Position::new(start as u32, 0),
                end: Position::new(end as u32, last_col as u32),
            },
            new_text: formatted,
        }]))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let mut actions = Vec::new();

        // Global Source Action: Format Document
        let wants_source_action = params.context.only.as_ref().map_or(true, |only| {
            only.contains(&CodeActionKind::SOURCE_FIX_ALL) || only.contains(&CodeActionKind::SOURCE)
        });

        if wants_source_action {
            if let Some(edits) = self.format_document(uri) {
                let mut changes = HashMap::new();
                changes.insert(uri.clone(), edits);
                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: "Format document with snapper".to_string(),
                    kind: Some(CodeActionKind::SOURCE_FIX_ALL),
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        ..Default::default()
                    }),
                    ..Default::default()
                }));
            }
        }

        let snapper_diags: Vec<&Diagnostic> = params
            .context
            .diagnostics
            .iter()
            .filter(|d| d.source.as_deref() == Some("snapper"))
            .collect();

        let docs = self.documents.lock().expect("document store poisoned");
        let Some((text, format)) = docs.get(uri) else {
            return Ok(if actions.is_empty() {
                None
            } else {
                Some(actions)
            });
        };

        let config = self.make_config(*format);

        for diag in &snapper_diags {
            let lines: Vec<&str> = text.lines().collect();
            let start = diag.range.start.line as usize;
            let end = diag.range.end.line as usize;

            if start >= lines.len() {
                continue;
            }

            let end = end.min(lines.len().saturating_sub(1));
            let range_text = lines[start..=end].join("\n");

            let formatted = match format_text(&range_text, &config) {
                Ok(f) => f,
                Err(_) => continue,
            };

            if formatted == range_text {
                continue;
            }

            let last_col = lines.get(end).map_or(0, |l| l.len());
            let edit = TextEdit {
                range: Range {
                    start: Position::new(start as u32, 0),
                    end: Position::new(end as u32, last_col as u32),
                },
                new_text: formatted,
            };

            let mut changes = HashMap::new();
            changes.insert(uri.clone(), vec![edit]);

            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: "Apply semantic line break".to_string(),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![(*diag).clone()]),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    ..Default::default()
                }),
                is_preferred: Some(true),
                ..Default::default()
            }));
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = &params.text_document.uri;
        let docs = self.documents.lock().expect("document store poisoned");
        let Some((_, format)) = docs.get(uri) else {
            return Ok(None);
        };

        let config = self.make_config(*format);
        let width_display = if config.max_width == 0 {
            "unlimited".to_string()
        } else {
            config.max_width.to_string()
        };

        let title = format!("snapper: {:?} | width: {}", config.format, width_display);

        Ok(Some(vec![CodeLens {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 0),
            },
            command: Some(Command {
                title,
                command: "snapper.showOutputChannel".to_string(),
                arguments: None,
            }),
            data: None,
        }]))
    }

    async fn on_type_formatting(
        &self,
        params: DocumentOnTypeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let position = params.text_document_position.position;
        self.range_formatting(DocumentRangeFormattingParams {
            text_document: params.text_document_position.text_document,
            range: Range {
                start: Position::new(position.line, 0),
                end: Position::new(position.line, position.character),
            },
            options: params.options,
            work_done_progress_params: Default::default(),
        })
        .await
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.lock().expect("document store poisoned");
        let Some((text, format)) = docs.get(uri) else {
            return Ok(None);
        };

        let line = text.lines().nth(pos.line as usize).unwrap_or("");
        let config = self.make_config(*format);
        let formatted = format_text(line, &config).unwrap_or_default();

        // Show hover preview only if the line actually needs formatting
        if formatted.trim() != line.trim() && formatted.lines().count() > 1 {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("**snapper preview:**\n```text\n{}\n```", formatted.trim()),
                }),
                range: None,
            }));
        }

        Ok(None)
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        if params.command == "snapper.reloadConfig" {
            self.reload_config();
            self.client
                .log_message(MessageType::INFO, "Manually reloaded .snapperrc.toml")
                .await;
        }
        Ok(None)
    }
}

fn detect_format_from_uri(uri: &Url, language_id: &str) -> Format {
    match language_id {
        "org" => return Format::Org,
        "latex" | "tex" => return Format::Latex,
        "markdown" => return Format::Markdown,
        "plaintext" => return Format::Plaintext,
        "restructuredtext" => return Format::Rst,
        _ => {}
    }
    if let Ok(path) = uri.to_file_path() {
        Format::from_path(&path)
    } else {
        Format::Plaintext
    }
}

/// Run the LSP server on stdin/stdout.
pub async fn run_lsp() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(SnapperLsp::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
