//! A real `snapper lsp` session over stdio, driven the way an editor
//! drives it. Positions are UTF-16 code units, as the protocol requires.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use serde_json::{Value, json};

const REPLY_TIMEOUT: Duration = Duration::from_secs(20);

struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    messages: Receiver<Value>,
    notifications: Vec<Value>,
    next_id: i64,
}

impl Session {
    fn start(root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_snapper"))
            .arg("lsp")
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start snapper lsp");
        let stdout = child.stdout.take().unwrap();
        let (tx, messages) = channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut length = None;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap_or(0) == 0 {
                        return;
                    }
                    let header = header.trim_end();
                    if header.is_empty() {
                        break;
                    }
                    if let Some(n) = header.strip_prefix("Content-Length: ") {
                        length = n.parse::<usize>().ok();
                    }
                }
                let mut body = vec![0; length.expect("Content-Length header")];
                if reader.read_exact(&mut body).is_err() {
                    return;
                }
                if tx.send(serde_json::from_slice(&body).unwrap()).is_err() {
                    return;
                }
            }
        });
        let stdin = child.stdin.take();
        let mut session = Session {
            child,
            stdin,
            messages,
            notifications: Vec::new(),
            next_id: 0,
        };
        let root_uri = format!("file://{}", root.display());
        session.request(
            "initialize",
            json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
        );
        session.notify("initialized", json!({}));
        session
    }

    fn send(&mut self, message: Value) {
        let body = message.to_string();
        let stdin = self.stdin.as_mut().expect("stdin is open");
        write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        stdin.flush().unwrap();
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let message = self
                .messages
                .recv_timeout(REPLY_TIMEOUT)
                .unwrap_or_else(|_| panic!("no reply to {method}"));
            if message["id"] == id && message.get("method").is_none() {
                return message;
            }
            self.notifications.push(message);
        }
    }

    fn wait_for_notification(&mut self, method: &str) -> Value {
        if let Some(i) = self
            .notifications
            .iter()
            .position(|m| m["method"] == method)
        {
            return self.notifications.remove(i);
        }
        loop {
            let message = self
                .messages
                .recv_timeout(REPLY_TIMEOUT)
                .unwrap_or_else(|_| panic!("no {method} notification"));
            if message["method"] == method {
                return message;
            }
            self.notifications.push(message);
        }
    }

    fn notified(&self, method: &str) -> Vec<&Value> {
        self.notifications
            .iter()
            .filter(|m| m["method"] == method)
            .collect()
    }

    fn open(&mut self, uri: &str, language_id: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri, "languageId": language_id, "version": 1, "text": text
            }}),
        );
    }

    fn format(&mut self, uri: &str) -> Value {
        self.request(
            "textDocument/formatting",
            json!({
                "textDocument": {"uri": uri},
                "options": {"tabSize": 4, "insertSpaces": true}
            }),
        )
    }

    fn format_range(&mut self, uri: &str, start: (u32, u32), end: (u32, u32)) -> Value {
        self.request(
            "textDocument/rangeFormatting",
            json!({
                "textDocument": {"uri": uri},
                "range": {
                    "start": {"line": start.0, "character": start.1},
                    "end": {"line": end.0, "character": end.1}
                },
                "options": {"tabSize": 4, "insertSpaces": true}
            }),
        )
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Byte offset of an LSP position, clamping the character to the line
/// length as the protocol tells clients to do.
fn offset_of(text: &str, position: &Value) -> usize {
    let line = position["line"].as_u64().unwrap() as usize;
    let character = position["character"].as_u64().unwrap() as usize;
    let mut start = 0;
    for _ in 0..line {
        match text[start..].find('\n') {
            Some(i) => start += i + 1,
            None => return text.len(),
        }
    }
    let rest = &text[start..];
    let line_text = rest[..rest.find('\n').unwrap_or(rest.len())].trim_end_matches('\r');
    let mut units = 0;
    for (i, c) in line_text.char_indices() {
        if units >= character {
            return start + i;
        }
        units += c.len_utf16();
    }
    start + line_text.len()
}

/// Apply the edits of a formatting reply, as an editor would.
fn apply(text: &str, reply: &Value) -> String {
    let Some(edits) = reply["result"].as_array() else {
        return text.to_string();
    };
    let mut spans: Vec<(usize, usize, &str)> = edits
        .iter()
        .map(|edit| {
            (
                offset_of(text, &edit["range"]["start"]),
                offset_of(text, &edit["range"]["end"]),
                edit["newText"].as_str().unwrap(),
            )
        })
        .collect();
    spans.sort_by_key(|&(start, _, _)| std::cmp::Reverse(start));
    let mut out = text.to_string();
    for (start, end, new_text) in spans {
        out.replace_range(start..end, new_text);
    }
    out
}

fn cli_format(dir: &Path, file_name: &str, text: &str) -> String {
    let path = dir.join(file_name);
    std::fs::write(&path, text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_snapper"))
        .current_dir(dir)
        .arg(file_name)
        .output()
        .expect("failed to run snapper");
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn document_formatting_matches_the_cli_at_the_end_of_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut lsp = Session::start(dir.path());
    let cases = [
        ("lf.md", "Hello. World.\n"),
        ("crlf.md", "Hello. World.\r\n"),
        ("none.md", "Hello. World."),
        (
            "emoji.md",
            "Hi \u{1f600}. Bye \u{1f600}.\n\nA \u{1f600}. B \u{4e2d}.",
        ),
    ];
    for (name, text) in cases {
        let uri = format!("file://{}/{name}", dir.path().display());
        lsp.open(&uri, "markdown", text);
        let formatted = apply(text, &lsp.format(&uri));
        assert_eq!(formatted, cli_format(dir.path(), name, text), "{name}");
    }
}

#[test]
fn range_formatting_outside_the_text_keeps_the_server_alive() {
    let dir = tempfile::tempdir().unwrap();
    let mut lsp = Session::start(dir.path());
    let empty = format!("file://{}/empty.md", dir.path().display());
    let short = format!("file://{}/short.md", dir.path().display());
    lsp.open(&empty, "markdown", "");
    lsp.open(&short, "markdown", "A one. B two.\n");

    assert_eq!(
        lsp.format_range(&empty, (0, 0), (0, 0))["result"],
        Value::Null
    );
    assert_eq!(
        lsp.format_range(&short, (3, 0), (4, 0))["result"],
        Value::Null
    );
    assert_eq!(
        apply("A one. B two.\n", &lsp.format(&short)),
        "A one.\nB two.\n"
    );
}

#[test]
fn range_ending_at_column_zero_leaves_that_line_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut lsp = Session::start(dir.path());
    let uri = format!("file://{}/a.md", dir.path().display());
    let text = "A one. B two.\nC three. D four.\n";
    lsp.open(&uri, "markdown", text);

    let reply = lsp.format_range(&uri, (0, 0), (1, 0));
    assert_eq!(apply(text, &reply), "A one.\nB two.\nC three. D four.\n");
}

#[test]
fn on_type_formatting_keeps_the_space_just_typed() {
    let dir = tempfile::tempdir().unwrap();
    let mut lsp = Session::start(dir.path());
    for (name, text, expected) in [
        ("a.md", "A one. B \nC.\n", "A one.\nB \nC.\n"),
        ("b.md", "A one. \nC.\n", "A one. \nC.\n"),
    ] {
        let uri = format!("file://{}/{name}", dir.path().display());
        lsp.open(&uri, "markdown", text);
        let character = text.find('\n').unwrap();
        let reply = lsp.request(
            "textDocument/onTypeFormatting",
            json!({
                "textDocument": {"uri": uri},
                "position": {"line": 0, "character": character},
                "ch": " ",
                "options": {"tabSize": 4, "insertSpaces": true}
            }),
        );
        assert_eq!(apply(text, &reply), expected, "typing a space in {text:?}");
    }
}

#[test]
fn range_and_quick_fix_edits_keep_crlf() {
    let dir = tempfile::tempdir().unwrap();
    let mut lsp = Session::start(dir.path());
    let uri = format!("file://{}/a.md", dir.path().display());
    let text = "A one. B two.\r\nC three.\r\n";
    let expected = "A one.\r\nB two.\r\nC three.\r\n";
    lsp.open(&uri, "markdown", text);

    let reply = lsp.format_range(&uri, (0, 0), (0, 13));
    assert_eq!(apply(text, &reply), expected);

    let fix = lsp.request(
        "textDocument/codeAction",
        json!({
            "textDocument": {"uri": uri},
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 13}},
            "context": {
                "only": ["quickfix"],
                "diagnostics": [{
                    "range": {"start": {"line": 0, "character": 0},
                              "end": {"line": 0, "character": 13}},
                    "source": "snapper",
                    "message": "fused: A one. B two."
                }]
            }
        }),
    );
    let edits = json!({"result": fix["result"][0]["edit"]["changes"][&uri]});
    assert_eq!(apply(text, &edits), expected);
}

#[test]
fn document_formatting_follows_the_project_lang_and_code_settings() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".snapperrc.toml"),
        "lang = \"de\"\n\n[code.python]\nline_comment = \"#\"\n",
    )
    .unwrap();
    let text = "Siehe Abb. 3 im Text. Gut.\n\n```python\n# First one. Second one.\nx = 1\n```\n";
    let cli = cli_format(dir.path(), "a.md", text);
    assert!(cli.starts_with("Siehe Abb. 3 im Text.\nGut.\n"), "{cli}");
    assert!(cli.contains("# First one.\n# Second one.\n"), "{cli}");

    let mut lsp = Session::start(dir.path());
    let uri = format!("file://{}/a.md", dir.path().display());
    lsp.open(&uri, "markdown", text);
    assert_eq!(apply(text, &lsp.format(&uri)), cli);
}

#[test]
fn malformed_config_is_reported_to_the_editor() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".snapperrc.toml"), "max_width = [\n").unwrap();
    let mut lsp = Session::start(dir.path());
    let uri = format!("file://{}/a.md", dir.path().display());
    lsp.open(&uri, "markdown", "A one.\n");
    lsp.format(&uri);

    let shown = lsp.notified("window/showMessage");
    assert!(
        shown.iter().any(|m| m["params"]["message"]
            .as_str()
            .unwrap()
            .contains(".snapperrc.toml")),
        "no message names the config file: {shown:?}"
    );
}

#[test]
fn diagnostic_covers_an_indented_line_with_an_emoji() {
    let dir = tempfile::tempdir().unwrap();
    let mut lsp = Session::start(dir.path());
    let uri = format!("file://{}/a.md", dir.path().display());
    let line = "  A \u{1f600} one. B two.";
    lsp.open(&uri, "markdown", &format!("- First item.\n{line}\n"));

    let published = lsp.wait_for_notification("textDocument/publishDiagnostics");
    let diagnostics = published["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let range = &diagnostics[0]["range"];
    assert_eq!(range["start"], json!({"line": 1, "character": 0}));
    assert_eq!(
        range["end"],
        json!({"line": 1, "character": line.encode_utf16().count()})
    );
}

#[test]
fn exit_ends_the_process_while_stdin_stays_open() {
    let dir = tempfile::tempdir().unwrap();
    let mut lsp = Session::start(dir.path());
    lsp.request("shutdown", Value::Null);
    lsp.notify("exit", Value::Null);

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = lsp.child.try_wait().unwrap() {
            break status;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "snapper lsp still running after exit"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success(), "{status:?}");
}
