use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::config::ProjectConfig;
use crate::format::Format;
use crate::{FormatConfig, format_text};

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
        let lines = text.lines().count();
        let last_line_len = text.lines().last().map_or(0, |l| l.len());
        Some(vec![TextEdit {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(lines as u32, last_line_len as u32),
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
        for (i, line) in text.lines().enumerate() {
            // Flag lines that contain multiple sentences (period + space + capital)
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut sentence_boundaries = 0;
            let chars: Vec<char> = trimmed.chars().collect();
            for j in 1..chars.len().saturating_sub(1) {
                if (chars[j - 1] == '.' || chars[j - 1] == '!' || chars[j - 1] == '?')
                    && chars[j] == ' '
                    && chars.get(j + 1).is_some_and(|c| c.is_uppercase())
                {
                    sentence_boundaries += 1;
                }
            }
            if sentence_boundaries >= 1 {
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position::new(i as u32, 0),
                        end: Position::new(i as u32, line.len() as u32),
                    },
                    severity: Some(DiagnosticSeverity::HINT),
                    source: Some("snapper".to_string()),
                    message: format!(
                        "Line contains {} sentence boundary(ies). Consider running snapper.",
                        sentence_boundaries
                    ),
                    ..Default::default()
                });
            }
        }
        diagnostics
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SnapperLsp {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Determine workspace root from init params
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
        // .snapperrc.toml changed -- reload config
        self.reload_config();
        self.client
            .log_message(MessageType::INFO, "Reloaded .snapperrc.toml")
            .await;

        // Recompute diagnostics for all open documents
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

        // Only offer code actions for snapper diagnostics
        let snapper_diags: Vec<&Diagnostic> = params
            .context
            .diagnostics
            .iter()
            .filter(|d| d.source.as_deref() == Some("snapper"))
            .collect();

        if snapper_diags.is_empty() {
            return Ok(None);
        }

        let docs = self.documents.lock().expect("document store poisoned");
        let Some((text, format)) = docs.get(uri) else {
            return Ok(None);
        };

        let config = self.make_config(*format);
        let mut actions = Vec::new();

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
                title: "Split into semantic line breaks".to_string(),
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
}

fn detect_format_from_uri(uri: &Url, language_id: &str) -> Format {
    // Try language ID first
    match language_id {
        "org" => return Format::Org,
        "latex" | "tex" => return Format::Latex,
        "markdown" => return Format::Markdown,
        "plaintext" => return Format::Plaintext,
        _ => {}
    }
    // Fall back to file extension
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
