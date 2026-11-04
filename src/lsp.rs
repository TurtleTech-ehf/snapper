use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::check::{DiagnosticKind, collect_diagnostics, resolve_long_threshold};
use crate::config::ProjectConfig;
use crate::format::Format;
use crate::{FormatConfig, build_splitter, format_text};

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
            latex_verbatim_envs: project.latex_verbatim_envs(),
            latex_structure_envs: project.latex_structure_envs(),
            latex_verbatim_commands: project.latex_verbatim_commands(),
            clause_breaks: project.clause_breaks.unwrap_or(false),
            lowercase_starts: project.lowercase_starts_for_format(format_str),
            // VS Code / Helix LSP stay native even if the CLI default flips.
            use_pandoc: false,
            ..Default::default()
        }
    }

    fn format_document(&self, uri: &Url) -> Option<Vec<TextEdit>> {
        // Returns None if lock is poisoned (graceful degradation)
        let docs = self.documents.lock().ok()?;
        let (text, format) = docs.get(uri)?;
        let config = self.make_config(*format);
        let formatted = format_text(text, &config).ok()?;
        whole_document_edit(text, formatted)
    }

    fn compute_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        let docs = self.documents.lock().expect("document store poisoned");
        let Some((text, format)) = docs.get(uri) else {
            return vec![];
        };
        let config = self.make_config(*format);
        let Ok(splitter) = build_splitter(&config) else {
            return vec![];
        };
        let threshold = {
            let project = self.project_config.lock().expect("config lock poisoned");
            resolve_long_threshold(config.max_width, project.long_threshold)
        };

        collect_diagnostics(text, *format, splitter.as_ref(), threshold, Some(&config))
            .into_iter()
            .map(|d| {
                let line = d.line.saturating_sub(1) as u32;
                let end_col = d.excerpt.chars().count() as u32;
                let severity = match d.kind {
                    DiagnosticKind::Long => DiagnosticSeverity::INFORMATION,
                    DiagnosticKind::Fused | DiagnosticKind::Wrap => DiagnosticSeverity::WARNING,
                };
                Diagnostic {
                    range: Range {
                        start: Position::new(line, 0),
                        end: Position::new(line, end_col),
                    },
                    severity: Some(severity),
                    source: Some("snapper".to_string()),
                    code: Some(NumberOrString::String(d.kind.as_str().to_string())),
                    message: format!("{}: {}", d.kind.as_str(), d.excerpt),
                    ..Default::default()
                }
            })
            .collect()
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

        let config = self.make_config(*format);
        // A range that already covers the whole buffer, final break
        // included, is the same edit as format document. Slicing through
        // `lines` drops that break and the replacement can put it back.
        if range_covers_document(text, range) {
            let formatted = match format_text(text, &config) {
                Ok(formatted) => formatted,
                Err(_) => return Ok(None),
            };
            return Ok(whole_document_edit(text, formatted));
        }

        let lines: Vec<&str> = text.lines().collect();
        let start = range.start.line as usize;
        let end = (range.end.line as usize).min(lines.len().saturating_sub(1));
        if start >= lines.len() || start > end {
            return Ok(None);
        }
        let range_text = lines[start..=end].join("\n");

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
        let wants_source_action = params.context.only.as_ref().is_none_or(|only| {
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

/// Position just past the last character, final line break included.
///
/// `str::lines` drops a final break, so a range built from the last
/// line's length stops before that break. The replacement from
/// `format_text` still carries the break when the input had one, and
/// the editor keeps the uncovered break, adding a line on every
/// reformat. A break is the boundary before the next line, so the
/// position after it is `(newline_count, 0)`.
fn document_end(text: &str) -> Position {
    let newlines = text.bytes().filter(|&b| b == b'\n').count();
    if text.ends_with('\n') {
        Position::new(newlines as u32, 0)
    } else {
        let last = text.rsplit_once('\n').map(|(_, last)| last).unwrap_or(text);
        let last = last.strip_suffix('\r').unwrap_or(last);
        // Byte length, matching the column this range used before.
        // A column past the UTF-16 line length clamps to the line end.
        Position::new(newlines as u32, last.len() as u32)
    }
}

fn range_covers_document(text: &str, range: Range) -> bool {
    let end = document_end(text);
    range.start.line == 0
        && range.start.character == 0
        && (range.end.line > end.line
            || (range.end.line == end.line && range.end.character >= end.character))
}

fn whole_document_edit(text: &str, formatted: String) -> Option<Vec<TextEdit>> {
    if formatted == text {
        None
    } else {
        Some(vec![TextEdit {
            range: Range {
                start: Position::new(0, 0),
                end: document_end(text),
            },
            new_text: formatted,
        }])
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
        Format::recognized_from_path(&path).unwrap_or(Format::Plaintext)
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::format::Format;
    use crate::{FormatConfig, format_text};

    fn markdown_config() -> FormatConfig {
        FormatConfig {
            format: Format::Markdown,
            use_pandoc: false,
            ..Default::default()
        }
    }

    fn final_newlines(s: &str) -> usize {
        s.bytes().rev().take_while(|&b| b == b'\n').count()
    }

    /// End position a client sends for the whole buffer.
    /// A final break ends at the start of the next line.
    fn client_document_end(text: &str) -> Position {
        let newlines = text.bytes().filter(|&b| b == b'\n').count();
        if text.ends_with('\n') {
            Position::new(newlines as u32, 0)
        } else {
            let last = text.rsplit_once('\n').map(|(_, last)| last).unwrap_or(text);
            let last = last.strip_suffix('\r').unwrap_or(last);
            let mut units = 0u32;
            for ch in last.chars() {
                units += ch.len_utf16() as u32;
            }
            Position::new(newlines as u32, units)
        }
    }

    /// Byte offset of an LSP position. Columns are UTF-16 code units.
    /// A column past the end of the line clamps to the line length.
    fn offset_at(text: &str, line: u32, character: u32) -> usize {
        let bytes = text.as_bytes();
        let mut line_start = 0usize;
        let mut seen = 0u32;
        let mut i = 0usize;
        while i < bytes.len() && seen < line {
            if bytes[i] == b'\n' {
                seen += 1;
                line_start = i + 1;
            }
            i += 1;
        }
        if seen < line {
            return text.len();
        }
        let mut line_end = line_start;
        while line_end < bytes.len() && bytes[line_end] != b'\n' {
            line_end += 1;
        }
        let mut content_end = line_end;
        if content_end > line_start && bytes[content_end - 1] == b'\r' {
            content_end -= 1;
        }
        let content = &text[line_start..content_end];
        let mut units = 0u32;
        for (idx, ch) in content.char_indices() {
            if units >= character {
                return line_start + idx;
            }
            units += ch.len_utf16() as u32;
        }
        line_start + content.len()
    }

    fn apply_edits(text: &str, edits: &[TextEdit]) -> String {
        let mut spans: Vec<(usize, usize, &str)> = edits
            .iter()
            .map(|edit| {
                let start = offset_at(text, edit.range.start.line, edit.range.start.character);
                let end = offset_at(text, edit.range.end.line, edit.range.end.character);
                (start, end, edit.new_text.as_str())
            })
            .collect();
        spans.sort_by_key(|span| std::cmp::Reverse(span.0));
        let mut out = text.to_string();
        for (start, end, new_text) in spans {
            assert!(start <= end, "edit range {start}..{end} is inverted");
            out.replace_range(start..end, new_text);
        }
        out
    }

    fn apply_optional(text: &str, edits: &Option<Vec<TextEdit>>) -> String {
        match edits {
            None => text.to_string(),
            Some(edits) => apply_edits(text, edits),
        }
    }

    struct Session {
        service: LspService<SnapperLsp>,
        socket: tower_lsp::ClientSocket,
        uri: Url,
    }

    impl Session {
        async fn open(text: &str) -> Self {
            let (service, socket) = LspService::new(SnapperLsp::new);
            let uri = Url::parse("file:///doc.md").expect("uri");
            service
                .inner()
                .did_open(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "markdown".into(),
                        version: 1,
                        text: text.to_string(),
                    },
                })
                .await;
            Self {
                service,
                socket,
                uri,
            }
        }

        fn server(&self) -> &SnapperLsp {
            self.service.inner()
        }

        fn options() -> FormattingOptions {
            FormattingOptions::default()
        }

        async fn formatting(&self) -> Option<Vec<TextEdit>> {
            self.server()
                .formatting(DocumentFormattingParams {
                    text_document: TextDocumentIdentifier {
                        uri: self.uri.clone(),
                    },
                    options: Self::options(),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                })
                .await
                .expect("formatting")
        }

        async fn range_formatting(&self, text: &str) -> Option<Vec<TextEdit>> {
            self.server()
                .range_formatting(DocumentRangeFormattingParams {
                    text_document: TextDocumentIdentifier {
                        uri: self.uri.clone(),
                    },
                    range: Range {
                        start: Position::new(0, 0),
                        end: client_document_end(text),
                    },
                    options: Self::options(),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                })
                .await
                .expect("range formatting")
        }

        async fn source_action(&self, text: &str) -> Option<Vec<TextEdit>> {
            let actions = self
                .server()
                .code_action(CodeActionParams {
                    text_document: TextDocumentIdentifier {
                        uri: self.uri.clone(),
                    },
                    range: Range {
                        start: Position::new(0, 0),
                        end: client_document_end(text),
                    },
                    context: CodeActionContext {
                        diagnostics: Vec::new(),
                        only: Some(vec![CodeActionKind::SOURCE]),
                        trigger_kind: None,
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                })
                .await
                .expect("code action");
            let actions = actions?;
            for action in actions {
                let CodeActionOrCommand::CodeAction(action) = action else {
                    continue;
                };
                let Some(edit) = action.edit else { continue };
                let Some(changes) = edit.changes else {
                    continue;
                };
                if let Some(edits) = changes.get(&self.uri) {
                    return Some(edits.clone());
                }
            }
            None
        }

        async fn quickfix_newline_count(&self, text: &str) -> usize {
            let diagnostics = self.server().compute_diagnostics(&self.uri);
            let actions = self
                .server()
                .code_action(CodeActionParams {
                    text_document: TextDocumentIdentifier {
                        uri: self.uri.clone(),
                    },
                    range: Range {
                        start: Position::new(0, 0),
                        end: client_document_end(text),
                    },
                    context: CodeActionContext {
                        diagnostics,
                        only: Some(vec![CodeActionKind::QUICKFIX]),
                        trigger_kind: None,
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                })
                .await
                .expect("quickfix");
            let Some(actions) = actions else {
                return final_newlines(text);
            };
            let mut count = final_newlines(text);
            for action in actions {
                let CodeActionOrCommand::CodeAction(action) = action else {
                    continue;
                };
                let Some(edit) = action.edit.as_ref() else {
                    continue;
                };
                let Some(changes) = edit.changes.as_ref() else {
                    continue;
                };
                let Some(edits) = changes.get(&self.uri) else {
                    continue;
                };
                count = count.max(final_newlines(&apply_edits(text, edits)));
            }
            count
        }
    }

    async fn assert_document(text: &str) {
        let cli = format_text(text, &markdown_config()).expect("cli format");
        let session = Session::open(text).await;
        let formatting = session.formatting().await;
        if text == "One. Two.\n" {
            let edits = formatting
                .as_ref()
                .expect("One. Two. with a final break needs one edit");
            assert_eq!(edits.len(), 1);
            assert_eq!(edits[0].range.start, Position::new(0, 0));
            assert_eq!(
                edits[0].range.end,
                Position::new(1, 0),
                "the edit must cover the final line break, got {:?}",
                edits[0].range.end
            );
            assert_eq!(edits[0].new_text, "One.\nTwo.\n");
        }
        let applied = apply_optional(text, &formatting);
        assert_eq!(applied, cli, "formatting vs format_text for {text:?}");
        assert_eq!(
            final_newlines(&applied),
            final_newlines(text),
            "final newlines for {text:?}, applied {applied:?}"
        );

        let again = Session::open(&applied).await;
        let second = apply_optional(&applied, &again.formatting().await);
        assert_eq!(second, applied, "formatting twice for {text:?}");

        let ranged = apply_optional(text, &session.range_formatting(text).await);
        assert_eq!(ranged, cli, "range formatting vs format_text for {text:?}");

        let source = apply_optional(text, &session.source_action(text).await);
        assert_eq!(source, cli, "source action vs format_text for {text:?}");

        let quick = session.quickfix_newline_count(text).await;
        assert!(
            quick <= final_newlines(text),
            "quickfix added a trailing newline for {text:?}: {quick}"
        );
        drop(session.socket);
    }

    #[test]
    fn formatting_edit_covers_the_final_line_break() {
        let cases = [
            "One. Two.\n",
            "One. Two.\n\nThree.\n",
            "- One. Two.\n\n",
            "One. Two.",
            "- One.\n  Two.\n",
            "One. Two.\r\n",
            "One. Two.\n\n",
            "",
            "café. Suite.\n",
            "café. Suite.",
        ];
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            for text in cases {
                assert_document(text).await;
            }
        });
    }
}

#[cfg(test)]
mod newline_cover {
    use super::*;
    use crate::format::Format;
    use tower_lsp::LanguageServer;
    use tower_lsp::lsp_types::{
        DidOpenTextDocumentParams, DocumentFormattingParams, FormattingOptions, Position,
        TextDocumentIdentifier, TextDocumentItem, TextEdit, Url, WorkDoneProgressParams,
    };

    fn cli_format(text: &str) -> String {
        format_text(
            text,
            &FormatConfig {
                format: Format::Markdown,
                ..FormatConfig::default()
            },
        )
        .expect("cli format")
    }

    fn trailing_newlines(text: &str) -> usize {
        text.bytes().rev().take_while(|b| *b == b'\n').count()
    }

    /// Byte index of an LSP position, using UTF-16 columns and `\n` / `\r\n` / `\r` breaks.
    /// A column past the end of a line clamps to the line length, as the spec requires.
    fn offset_at(text: &str, pos: Position) -> usize {
        let mut line_start = 0usize;
        let mut line = 0u32;
        let bytes = text.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() && line < pos.line {
            if bytes[i] == b'\n' {
                i += 1;
                line += 1;
                line_start = i;
            } else if bytes[i] == b'\r' {
                i += 1;
                if i < bytes.len() && bytes[i] == b'\n' {
                    i += 1;
                }
                line += 1;
                line_start = i;
            } else {
                i += 1;
            }
        }
        if line < pos.line {
            return text.len();
        }
        let mut content_end = line_start;
        while content_end < bytes.len()
            && bytes[content_end] != b'\n'
            && bytes[content_end] != b'\r'
        {
            content_end += 1;
        }
        let content = &text[line_start..content_end];
        let mut seen = 0u32;
        for (idx, ch) in content.char_indices() {
            if seen >= pos.character {
                return line_start + idx;
            }
            let width = ch.len_utf16() as u32;
            if seen + width > pos.character {
                return line_start + idx;
            }
            seen += width;
        }
        content_end
    }

    fn apply_text_edits(text: &str, edits: &[TextEdit]) -> String {
        let mut ordered = edits.to_vec();
        ordered.sort_by(|a, b| {
            b.range
                .start
                .line
                .cmp(&a.range.start.line)
                .then(b.range.start.character.cmp(&a.range.start.character))
        });
        let mut result = text.to_string();
        for edit in ordered {
            let start = offset_at(&result, edit.range.start);
            let end = offset_at(&result, edit.range.end);
            assert!(start <= end, "inverted edit range");
            result.replace_range(start..end, &edit.new_text);
        }
        result
    }

    async fn format_once(text: &str) -> String {
        let (service, _socket) = LspService::new(SnapperLsp::new);
        let server = service.inner();
        let uri = Url::parse("file:///note.md").expect("uri");
        server
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem::new(
                    uri.clone(),
                    "markdown".to_string(),
                    1,
                    text.to_string(),
                ),
            })
            .await;
        let edits = server
            .formatting(DocumentFormattingParams {
                text_document: TextDocumentIdentifier::new(uri),
                options: FormattingOptions {
                    tab_size: 2,
                    insert_spaces: true,
                    ..FormattingOptions::default()
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await
            .expect("formatting");
        if let Some(edits) = &edits {
            assert_eq!(
                edits.len(),
                1,
                "document formatting is one full-buffer edit"
            );
            let edit = &edits[0];
            assert_eq!(offset_at(text, edit.range.start), 0);
            assert_eq!(
                offset_at(text, edit.range.end),
                text.len(),
                "edit range must include a trailing newline; new_text already ends with one\n\
                 input: {text:?}\nedit end: {:?}\nnew_text: {:?}",
                edit.range.end,
                edit.new_text
            );
            assert_eq!(edit.new_text, cli_format(text));
        }
        apply_text_edits(text, edits.as_deref().unwrap_or(&[]))
    }

    #[test]
    fn client_offset_includes_the_final_newline() {
        assert_eq!(offset_at("One. Two.\n", Position::new(0, 9)), 9);
        assert_eq!(offset_at("One. Two.\n", Position::new(1, 0)), 10);
        assert_eq!(offset_at("One. Two.", Position::new(0, 9)), 9);
        assert_eq!(offset_at("One. Two.\n\n", Position::new(2, 0)), 11);
        assert_eq!(offset_at("One. Two.\r\n", Position::new(1, 0)), 11);
    }

    /// `textDocument/formatting` applied twice must match the CLI formatter.
    /// Inputs cover zero, one, and several trailing newlines. A document that
    /// ends in one newline must still end in exactly one after formatting.
    #[tokio::test]
    async fn formatting_twice_matches_cli_and_keeps_one_final_newline() {
        let cases = [
            "One. Two.",
            "One. Two.\n",
            "One. Two.\n\n",
            "One. Two.\n\n\n",
            "One. Two.\n\nThree.\n",
            "- One. Two.\n\n",
            "- One.\n  Two.\n",
            "One. Two.\r\n",
            "",
            "\n",
            "\n\n",
        ];
        for input in cases {
            let cli = cli_format(input);
            let once = format_once(input).await;
            let twice = format_once(&once).await;
            assert_eq!(
                once, cli,
                "first LSP format is not the CLI output for {input:?}"
            );
            assert_eq!(
                twice, cli,
                "second LSP format is not the CLI output for {input:?}"
            );
            if trailing_newlines(input) == 1 && !input.contains('\r') {
                assert_eq!(
                    trailing_newlines(&twice),
                    1,
                    "expected exactly one final newline for {input:?}, got {twice:?}"
                );
            }
            if !input.ends_with('\n') && !input.ends_with('\r') {
                assert!(
                    !twice.ends_with('\n') && !twice.ends_with('\r'),
                    "a file with no final newline must stay that way, got {twice:?}"
                );
            }
            assert_eq!(
                trailing_newlines(&twice),
                trailing_newlines(&cli),
                "LSP trailing newlines diverged from the CLI for {input:?}"
            );
        }

        let reproduced = format_once("One. Two.\n").await;
        let reproduced_again = format_once(&reproduced).await;
        assert_eq!(reproduced, "One.\nTwo.\n");
        assert_eq!(reproduced_again, "One.\nTwo.\n");
        assert_eq!(trailing_newlines(&reproduced_again), 1);
    }
}
