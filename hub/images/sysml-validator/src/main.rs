//! sysml-validator: OpenSpoke の知識ベースに書く SysML v2 本文を調べる HTTP サービス。
//!
//! 2 段で調べる。片方だけでは取りこぼしがあるため、両方に通ったものだけを合格とする。
//!   1. 構文チェッカー sysml2 (tree-sitter による構文解析。参照の解決は見ない)
//!   2. バリデーター mercurio-sysml (構文と、型・再定義先・式中の名前の解決)
//!
//! 口:
//!   GET  /healthz   → 200 "ok"
//!   POST /validate  → 本文 {"content": "<SysML v2 テキスト>"}
//!                     返り {"ok", "syntax": {...}, "semantic": {...}, "tools": {...}}
//!
//! kernel の知識ベース書き込み (backends.py の topic_upsert) から呼ばれる。
//! AI が任意に呼ぶものではなく、書き込みの経路に組み込まれた仕組みとして使う。

use mercurio_sysml::SysmlEnvironment;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::time::Instant;
use tiny_http::{Header, Method, Response, Server};

const SYSML2_VERSION: &str = "0.3.1";
const MERCURIO_VERSION: &str = "0.86.0";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Deserialize)]
struct ValidateRequest {
    content: String,
}

#[derive(Serialize)]
struct Diagnostic {
    line: usize,
    column: usize,
    code: String,
    message: String,
}

#[derive(Serialize)]
struct StageResult {
    ok: bool,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Serialize)]
struct ValidateResponse {
    ok: bool,
    syntax: StageResult,
    semantic: StageResult,
    elements: Option<usize>,
    elapsed_ms: u128,
    tools: serde_json::Value,
}

fn tools() -> serde_json::Value {
    serde_json::json!({
        "syntax_checker": {"name": "sysml2", "version": SYSML2_VERSION},
        "validator": {"name": "mercurio-sysml", "version": MERCURIO_VERSION},
    })
}

/// 構文チェッカー。sysml2 はファイル経路を受け取るので、一時ディレクトリーに書いて渡す。
fn check_syntax(content: &str) -> StageResult {
    let dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => return internal_error("syntax.io", format!("一時ディレクトリーを作れない: {e}")),
    };
    let path = dir.path().join("topic.sysml");
    if let Err(e) = std::fs::File::create(&path).and_then(|mut f| f.write_all(content.as_bytes())) {
        return internal_error("syntax.io", format!("一時ファイルに書けない: {e}"));
    }
    match sysml::check_paths(&[path]) {
        Ok(report) => {
            let diagnostics: Vec<Diagnostic> = report
                .files
                .into_iter()
                .flat_map(|f| f.diagnostics)
                .map(|d| Diagnostic {
                    line: d.span.start_line,
                    column: d.span.start_column,
                    code: d.code,
                    message: d.message,
                })
                .collect();
            StageResult { ok: report.valid && diagnostics.is_empty(), diagnostics }
        }
        Err(e) => internal_error("syntax.error", e.to_string()),
    }
}

/// バリデーター。mercurio-sysml は最初の診断 1 件を返す。
fn check_semantic(env: &SysmlEnvironment, content: &str) -> (StageResult, Option<usize>) {
    match env.compile_text(content, "topic.sysml") {
        Ok(doc) => (StageResult { ok: true, diagnostics: vec![] }, Some(doc.elements.len())),
        Err(e) => {
            // 診断は型で取り出す (Debug 表現の文字列を解析すると版が変わったときに壊れる)。
            let diag = match e {
                mercurio_sysml::SysmlEnvironmentError::Diagnostic(d) => {
                    let (line, column) = d
                        .span
                        .as_ref()
                        .map(|s| (s.start_line, s.start_col))
                        .unwrap_or((0, 0));
                    Diagnostic {
                        line,
                        column,
                        code: format!("semantic.{:?}", d.kind).to_lowercase(),
                        message: d.message.clone(),
                    }
                }
                other => Diagnostic { line: 0, column: 0, code: "semantic.error".into(), message: format!("{other:?}") },
            };
            (StageResult { ok: false, diagnostics: vec![diag] }, None)
        }
    }
}

fn internal_error(code: &str, message: String) -> StageResult {
    StageResult { ok: false, diagnostics: vec![Diagnostic { line: 0, column: 0, code: code.into(), message }] }
}

fn json_response(status: u16, body: &impl Serialize) -> Response<std::io::Cursor<Vec<u8>>> {
    let bytes = serde_json::to_vec(body).unwrap_or_else(|_| b"{\"ok\":false}".to_vec());
    Response::from_data(bytes)
        .with_status_code(status)
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap())
}

fn main() {
    let addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let env = SysmlEnvironment::latest_metamodel().expect("SysML 環境の初期化に失敗");
    let server = Server::http(&addr).expect("待ち受けに失敗");
    eprintln!("[sysml-validator] listening on {addr} (sysml2 {SYSML2_VERSION}, mercurio-sysml {MERCURIO_VERSION})");

    for mut req in server.incoming_requests() {
        let path = req.url().split('?').next().unwrap_or("").to_string();
        let resp = match (req.method(), path.as_str()) {
            (Method::Get, "/healthz") => Response::from_string("ok").with_status_code(200),
            (Method::Post, "/validate") => {
                let started = Instant::now();
                let mut raw = Vec::new();
                let read = req.as_reader().take(MAX_BODY_BYTES as u64 + 1).read_to_end(&mut raw);
                let parsed: Result<ValidateRequest, String> = match read {
                    Err(e) => Err(format!("本文を読めない: {e}")),
                    Ok(_) if raw.len() > MAX_BODY_BYTES => Err(format!("本文が大きすぎる (上限 {MAX_BODY_BYTES} バイト)")),
                    Ok(_) => serde_json::from_slice(&raw).map_err(|e| format!("JSON として読めない: {e}")),
                };
                match parsed {
                    Err(msg) => {
                        let _ = req.respond(json_response(400, &serde_json::json!({"ok": false, "error": msg})));
                        continue;
                    }
                    Ok(body) => {
                        let syntax = check_syntax(&body.content);
                        let (semantic, elements) = check_semantic(&env, &body.content);
                        let out = ValidateResponse {
                            ok: syntax.ok && semantic.ok,
                            syntax,
                            semantic,
                            elements,
                            elapsed_ms: started.elapsed().as_millis(),
                            tools: tools(),
                        };
                        eprintln!(
                            "[sysml-validator] validate ok={} syntax={} semantic={} bytes={} ms={}",
                            out.ok, out.syntax.ok, out.semantic.ok, body.content.len(), out.elapsed_ms
                        );
                        let _ = req.respond(json_response(200, &out));
                        continue;
                    }
                }
            }
            _ => Response::from_string("not found").with_status_code(404),
        };
        let _ = req.respond(resp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// tests/cases の ok_* は両方に合格し、ng_* はどちらかで落ちること。
    /// ng_* が 1 件でも通ったら、仕組みとして役に立たないのでイメージを作らせない。
    #[test]
    fn cases() {
        let env = SysmlEnvironment::latest_metamodel().unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
        let mut seen = (0, 0);
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let content = std::fs::read_to_string(&path).unwrap();
            let ok = check_syntax(&content).ok && check_semantic(&env, &content).0.ok;
            if name.starts_with("ok_") {
                assert!(ok, "{name} は合格するはず");
                seen.0 += 1;
            } else if name.starts_with("ng_") {
                assert!(!ok, "{name} は落ちるはず");
                seen.1 += 1;
            }
        }
        // 実物のモデルの参照を 1 か所だけ壊したもの。どれもバリデーターで落ちること。
        let base = std::fs::read_to_string(dir.join("ok_fleet_gitrepos.sysml")).unwrap();
        for (from, to) in [
            (":>> targets = rke2;", ":>> targets = rke3;"),
            ("repo_gitlab_example;", "repo_gitlab_examplea;"),
            ("State::ready;", "State::redy;"),
            (":>> pollingInterval = \"5m0s\";", ":>> pollingIntervall = \"5m0s\";"),
        ] {
            assert!(base.contains(from), "置き換え元が無い: {from}");
            let broken = base.replacen(from, to, 1);
            assert!(!check_semantic(&env, &broken).0.ok, "{from} → {to} は落ちるはず");
            seen.1 += 1;
        }
        assert!(seen.0 >= 3 && seen.1 >= 9, "試験の材料が足りない: {seen:?}");
    }
}
