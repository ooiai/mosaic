use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use mosaic_agent::AgentRunOptions;
use mosaic_core::error::{MosaicError, Result};
use mosaic_mcp::{McpStore, mcp_servers_file_path};
use mosaic_memory::{
    MemoryIndexOptions, MemoryIndexResult, MemorySearchHit, MemoryStore,
    list_memory_namespace_statuses, memory_index_path_for_namespace,
    memory_status_path_for_namespace,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::sleep;
use walkdir::WalkDir;

use super::utils::resolve_output_path;
use super::{
    Cli, KnowledgeArgs, KnowledgeCommand, KnowledgeDatasetsCommand, KnowledgeSourceArg,
    build_runtime, print_json, resolve_state_paths,
};

const KNOWLEDGE_EVAL_BASELINE_VERSION: u32 = 1;
const KNOWLEDGE_EVAL_HISTORY_VERSION: u32 = 1;

pub(super) async fn handle_knowledge(cli: &Cli, args: KnowledgeArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;

    match args.command {
        KnowledgeCommand::Ingest {
            source,
            path,
            urls,
            url_file,
            continue_on_error,
            report_out,
            headers,
            header_envs,
            mcp_server,
            mcp_path,
            namespace,
            max_chunk_bytes,
            chunk_overlap_bytes,
            incremental,
            stale_after_hours,
            retain_missing,
            max_files,
            max_file_size,
            max_content_bytes,
            http_timeout_seconds,
            http_retries,
            http_retry_backoff_ms,
        } => {
            let namespace = normalize_namespace(&namespace)?;
            let max_chunk_bytes = ensure_positive_usize("max_chunk_bytes", max_chunk_bytes)?;
            let chunk_overlap_bytes =
                ensure_non_negative_overlap(chunk_overlap_bytes, max_chunk_bytes)?;
            let max_files = ensure_positive_usize("max_files", max_files)?;
            let max_file_size = ensure_positive_usize("max_file_size", max_file_size)?;
            let max_content_bytes = ensure_positive_usize("max_content_bytes", max_content_bytes)?;
            let http_timeout_seconds =
                ensure_positive_u64("http_timeout_seconds", http_timeout_seconds)?;
            let http_retries = ensure_positive_u32("http_retries", http_retries)?;
            let http_retry_backoff_ms =
                ensure_positive_u64("http_retry_backoff_ms", http_retry_backoff_ms)?;
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let report_out_path = report_out
                .as_deref()
                .map(|raw| resolve_output_path(&cwd, raw));
            let is_http_source = matches!(source, KnowledgeSourceArg::Http);
            if !is_http_source {
                if continue_on_error {
                    return Err(MosaicError::Validation(
                        "--continue-on-error is only supported with --source http".to_string(),
                    ));
                }
                if !headers.is_empty() || !header_envs.is_empty() {
                    return Err(MosaicError::Validation(
                        "--header/--header-env are only supported with --source http".to_string(),
                    ));
                }
            }

            let stage_root = knowledge_stage_root(&paths.root_dir, &namespace, source);
            std::fs::create_dir_all(&stage_root)?;
            let mut expected_files = HashSet::new();
            let mut stats = IngestStats::default();
            let mut http_stats: Option<HttpIngestStats> = None;
            let stage_options = StageOptions {
                max_chunk_bytes,
                chunk_overlap_bytes,
                max_file_size,
                max_files,
            };

            let source_detail = match source {
                KnowledgeSourceArg::LocalMd => {
                    let path = path.unwrap_or_else(|| ".".to_string());
                    let root = resolve_source_root(&path)?;
                    stage_markdown_from_directory(
                        &root,
                        &stage_root,
                        "local_md",
                        &stage_options,
                        &mut expected_files,
                        &mut stats,
                    )?;
                    json!({
                        "path": root.display().to_string(),
                    })
                }
                KnowledgeSourceArg::Http => {
                    let urls = collect_http_urls(urls, url_file)?;
                    if urls.is_empty() {
                        return Err(MosaicError::Validation(
                            "knowledge ingest --source http requires at least one --url or --url-file entry".to_string(),
                        ));
                    }
                    let (http_headers, header_names) = parse_http_headers(headers, header_envs)?;
                    let gathered = stage_markdown_from_http(
                        &urls,
                        &stage_root,
                        http_timeout_seconds,
                        &http_headers,
                        http_retries,
                        http_retry_backoff_ms,
                        continue_on_error,
                        &stage_options,
                        &mut expected_files,
                        &mut stats,
                    )
                    .await?;
                    let succeeded = gathered.succeeded;
                    let failed = gathered.failed;
                    let total = gathered.entries.len();
                    http_stats = Some(gathered);
                    json!({
                        "urls": urls,
                        "headers": header_names,
                        "retry": {
                            "attempts": http_retries,
                            "base_backoff_ms": http_retry_backoff_ms,
                        },
                        "continue_on_error": continue_on_error,
                        "fetched_total": total,
                        "fetched_succeeded": succeeded,
                        "fetched_failed": failed,
                        "report_out": report_out_path.as_ref().map(|path| path.display().to_string()),
                    })
                }
                KnowledgeSourceArg::Mcp => {
                    let server_id = mcp_server.ok_or_else(|| {
                        MosaicError::Validation(
                            "knowledge ingest --source mcp requires --mcp-server".to_string(),
                        )
                    })?;
                    let root = resolve_mcp_markdown_root(&paths.data_dir, &server_id, mcp_path)?;
                    stage_markdown_from_directory(
                        &root,
                        &stage_root,
                        "mcp",
                        &stage_options,
                        &mut expected_files,
                        &mut stats,
                    )?;
                    json!({
                        "mcp_server": server_id,
                        "path": root.display().to_string(),
                    })
                }
            };

            stats.stale_removed = remove_stale_stage_files(&stage_root, &expected_files)?;

            let store = MemoryStore::new(
                memory_index_path_for_namespace(&paths.data_dir, &namespace),
                memory_status_path_for_namespace(&paths.data_dir, &namespace),
            );
            let index = store.index(MemoryIndexOptions {
                root: stage_root.clone(),
                incremental,
                stale_after_hours,
                retain_missing,
                max_files,
                max_file_size,
                max_content_bytes,
            })?;

            if let Some(path) = report_out_path.as_ref() {
                save_knowledge_ingest_report(
                    path,
                    knowledge_source_name(source),
                    &namespace,
                    &source_detail,
                    &stats,
                    &index,
                    http_stats.as_ref(),
                    http_retries,
                    http_retry_backoff_ms,
                )?;
            }

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "source": knowledge_source_name(source),
                    "source_detail": source_detail,
                    "namespace": namespace,
                    "stage_root": stage_root.display().to_string(),
                    "documents_seen": stats.documents_seen,
                    "documents_skipped": stats.documents_skipped,
                    "chunks_written": stats.chunks_written,
                    "chunks_reused": stats.chunks_reused,
                    "stale_removed": stats.stale_removed,
                    "http": http_stats.as_ref().map(|value| json!({
                        "fetched_total": value.entries.len(),
                        "fetched_succeeded": value.succeeded,
                        "fetched_failed": value.failed,
                    })),
                    "index": index,
                }));
            } else {
                println!("knowledge ingest: {}", knowledge_source_name(source));
                println!("namespace: {}", namespace);
                println!("stage root: {}", stage_root.display());
                println!("documents seen: {}", stats.documents_seen);
                println!("documents skipped: {}", stats.documents_skipped);
                println!("chunks written: {}", stats.chunks_written);
                println!("chunks reused: {}", stats.chunks_reused);
                println!("stale chunks removed: {}", stats.stale_removed);
                if let Some(value) = http_stats.as_ref() {
                    println!("http fetched total: {}", value.entries.len());
                    println!("http fetched succeeded: {}", value.succeeded);
                    println!("http fetched failed: {}", value.failed);
                }
                if let Some(path) = report_out_path.as_ref() {
                    println!("ingest report: {}", path.display());
                }
                println!("indexed documents: {}", index.indexed_documents);
                println!("reused documents: {}", index.reused_documents);
                println!("reindexed documents: {}", index.reindexed_documents);
                println!("removed documents: {}", index.removed_documents);
            }
        }
        KnowledgeCommand::Search {
            query,
            namespace,
            limit,
            min_score,
        } => {
            let namespace = normalize_namespace(&namespace)?;
            let limit = ensure_positive_usize("limit", limit)?;
            let query = normalize_non_empty("query", &query)?;
            let hits =
                search_hits_for_namespace(&paths.data_dir, &namespace, &query, limit, min_score)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "query": query,
                    "namespace": namespace,
                    "min_score": min_score,
                    "total_hits": hits.len(),
                    "hits": hits,
                }));
            } else if hits.is_empty() {
                println!("No knowledge hits.");
            } else {
                for hit in hits {
                    println!("- {} (score={}): {}", hit.path, hit.score, hit.snippet);
                }
            }
        }
        KnowledgeCommand::Ask {
            prompt,
            namespace,
            top_k,
            min_score,
            references_only,
            session,
            agent,
        } => {
            let namespace = normalize_namespace(&namespace)?;
            let top_k = ensure_positive_usize("top_k", top_k)?;
            let prompt = normalize_non_empty("prompt", &prompt)?;
            let hits =
                search_hits_for_namespace(&paths.data_dir, &namespace, &prompt, top_k, min_score)?;
            if references_only {
                if session.is_some() || agent.is_some() {
                    return Err(MosaicError::Validation(
                        "--references-only cannot be combined with --session or --agent"
                            .to_string(),
                    ));
                }
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "mode": "references_only",
                        "namespace": namespace,
                        "top_k": top_k,
                        "min_score": min_score,
                        "prompt": prompt,
                        "total_references": hits.len(),
                        "references": hits,
                    }));
                } else if hits.is_empty() {
                    println!("No knowledge references.");
                } else {
                    println!("knowledge references:");
                    for hit in hits {
                        println!("- {} (score={}): {}", hit.path, hit.score, hit.snippet);
                    }
                }
                return Ok(());
            }
            let augmented_prompt = render_augmented_prompt(&prompt, &hits);

            let runtime = build_runtime(cli, agent.as_deref(), Some("knowledge.ask"))?;
            let result = runtime
                .agent
                .ask(
                    &augmented_prompt,
                    AgentRunOptions {
                        session_id: session,
                        cwd: std::env::current_dir()
                            .map_err(|err| MosaicError::Io(err.to_string()))?,
                        yes: cli.yes,
                        interactive: false,
                    },
                )
                .await?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "namespace": namespace,
                    "top_k": top_k,
                    "min_score": min_score,
                    "session_id": result.session_id,
                    "response": result.response,
                    "turns": result.turns,
                    "references": hits,
                    "agent_id": runtime.active_agent_id,
                    "profile": runtime.active_profile_name,
                }));
            } else {
                println!("{}", result.response.trim());
                println!("session: {}", result.session_id);
                if !hits.is_empty() {
                    println!("knowledge references:");
                    for hit in hits {
                        println!("- {} (score={})", hit.path, hit.score);
                    }
                }
            }
        }
        KnowledgeCommand::Evaluate {
            queries,
            query_file,
            namespace,
            top_k,
            min_score,
            baseline,
            no_baseline,
            update_baseline,
            fail_on_regression,
            max_coverage_drop,
            max_avg_top_score_drop,
            history_window,
            report_out,
        } => {
            let namespace = normalize_namespace(&namespace)?;
            let top_k = ensure_positive_usize("top_k", top_k)?;
            let history_window = ensure_positive_usize("history_window", history_window)?;
            if no_baseline && update_baseline {
                return Err(MosaicError::Validation(
                    "--no-baseline and --update-baseline cannot be used together".to_string(),
                ));
            }
            if max_coverage_drop < 0.0 {
                return Err(MosaicError::Validation(
                    "--max-coverage-drop must be greater than or equal to 0".to_string(),
                ));
            }
            if max_avg_top_score_drop < 0.0 {
                return Err(MosaicError::Validation(
                    "--max-avg-top-score-drop must be greater than or equal to 0".to_string(),
                ));
            }
            let query_list = collect_evaluation_queries(queries, query_file)?;
            if query_list.is_empty() {
                return Err(MosaicError::Validation(
                    "knowledge evaluate requires at least one --query or --query-file entry"
                        .to_string(),
                ));
            }

            let mut rows = Vec::new();
            let mut with_hits = 0usize;
            let mut total_hits = 0usize;
            let mut top_scores = Vec::new();
            for query in &query_list {
                let hits = search_hits_for_namespace(
                    &paths.data_dir,
                    &namespace,
                    query,
                    top_k,
                    min_score,
                )?;
                let hit_count = hits.len();
                total_hits = total_hits.saturating_add(hit_count);
                if hit_count > 0 {
                    with_hits = with_hits.saturating_add(1);
                    top_scores.push(hits[0].score);
                }
                rows.push(KnowledgeEvalQueryResult {
                    query: query.clone(),
                    hit_count,
                    top_score: hits.first().map(|hit| hit.score).unwrap_or(0),
                    top_paths: hits.into_iter().take(3).map(|hit| hit.path).collect(),
                });
            }

            let query_total = query_list.len();
            let without_hits = query_total.saturating_sub(with_hits);
            let summary = KnowledgeEvalSummary {
                query_total,
                with_hits,
                without_hits,
                coverage_rate: if query_total == 0 {
                    0.0
                } else {
                    (with_hits as f64) / (query_total as f64)
                },
                avg_hits: if query_total == 0 {
                    0.0
                } else {
                    (total_hits as f64) / (query_total as f64)
                },
                avg_top_score: if top_scores.is_empty() {
                    0.0
                } else {
                    (top_scores.iter().sum::<usize>() as f64) / (top_scores.len() as f64)
                },
                p50_top_score: percentile_score(&top_scores, 50),
                p90_top_score: percentile_score(&top_scores, 90),
            };

            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let baseline_path = baseline
                .as_deref()
                .map(|raw| resolve_output_path(&cwd, raw))
                .unwrap_or_else(|| {
                    default_knowledge_eval_baseline_path(&paths.data_dir, &namespace)
                });
            let history_path = default_knowledge_eval_history_path(&paths.data_dir, &namespace);

            let baseline_record = if no_baseline {
                None
            } else {
                load_knowledge_eval_baseline(&baseline_path)?
            };
            let baseline_enabled = !no_baseline;
            let mut baseline_found = baseline_record.is_some();
            let mut baseline_updated = false;
            let mut baseline_regressed = false;
            let mut baseline_reasons = Vec::new();
            let mut coverage_delta = None;
            let mut avg_top_score_delta = None;
            let mut baseline_summary: Option<KnowledgeEvalSummary> = None;

            if let Some(value) = baseline_record.as_ref() {
                baseline_summary = Some(value.summary.clone());
                let computed_coverage_delta = summary.coverage_rate - value.summary.coverage_rate;
                let computed_avg_top_score_delta =
                    summary.avg_top_score - value.summary.avg_top_score;
                coverage_delta = Some(computed_coverage_delta);
                avg_top_score_delta = Some(computed_avg_top_score_delta);
                if computed_coverage_delta < -max_coverage_drop {
                    baseline_regressed = true;
                    baseline_reasons.push(format!(
                        "coverage_rate dropped by {:.4} (baseline={:.4}, current={:.4}, allowed_drop={:.4})",
                        value.summary.coverage_rate - summary.coverage_rate,
                        value.summary.coverage_rate,
                        summary.coverage_rate,
                        max_coverage_drop
                    ));
                }
                if computed_avg_top_score_delta < -max_avg_top_score_drop {
                    baseline_regressed = true;
                    baseline_reasons.push(format!(
                        "avg_top_score dropped by {:.4} (baseline={:.4}, current={:.4}, allowed_drop={:.4})",
                        value.summary.avg_top_score - summary.avg_top_score,
                        value.summary.avg_top_score,
                        summary.avg_top_score,
                        max_avg_top_score_drop
                    ));
                }
            }

            if update_baseline {
                let next_baseline = KnowledgeEvalBaseline {
                    version: KNOWLEDGE_EVAL_BASELINE_VERSION,
                    namespace: namespace.clone(),
                    top_k,
                    min_score,
                    created_unix_ms: now_unix_ms(),
                    summary: summary.clone(),
                    queries: rows.clone(),
                };
                save_knowledge_eval_baseline(&baseline_path, &next_baseline)?;
                baseline_summary = Some(next_baseline.summary);
                baseline_found = true;
                baseline_updated = true;
            }

            let history_entries_before = load_knowledge_eval_history(&history_path)?;
            let previous_history = history_entries_before.last().cloned();
            let window_before =
                history_window_stats(history_tail(&history_entries_before, history_window));
            let history_entry = KnowledgeEvalHistoryEntry {
                version: KNOWLEDGE_EVAL_HISTORY_VERSION,
                ts_unix_ms: now_unix_ms(),
                namespace: namespace.clone(),
                top_k,
                min_score,
                baseline_regressed,
                summary: summary.clone(),
            };
            append_knowledge_eval_history(&history_path, &history_entry)?;
            let mut history_entries_after = history_entries_before.clone();
            history_entries_after.push(history_entry.clone());
            let window_with_current =
                history_window_stats(history_tail(&history_entries_after, history_window));
            let history_delta_vs_previous = previous_history.as_ref().map(|value| {
                json!({
                    "coverage_rate": summary.coverage_rate - value.summary.coverage_rate,
                    "avg_hits": summary.avg_hits - value.summary.avg_hits,
                    "avg_top_score": summary.avg_top_score - value.summary.avg_top_score,
                    "p50_top_score": summary.p50_top_score as i64 - value.summary.p50_top_score as i64,
                    "p90_top_score": summary.p90_top_score as i64 - value.summary.p90_top_score as i64,
                })
            });
            let history_previous = previous_history.as_ref().map(|value| {
                json!({
                    "ts_unix_ms": value.ts_unix_ms,
                    "summary": &value.summary,
                })
            });

            let report_out_path = report_out
                .as_deref()
                .map(|raw| resolve_output_path(&cwd, raw));

            let payload = json!({
                "ok": true,
                "namespace": namespace,
                "top_k": top_k,
                "min_score": min_score,
                "summary": &summary,
                "queries": &rows,
                "baseline": {
                    "enabled": baseline_enabled,
                    "found": baseline_found,
                    "path": baseline_path.display().to_string(),
                    "updated": baseline_updated,
                    "regressed": baseline_regressed,
                    "reasons": baseline_reasons.clone(),
                    "thresholds": {
                        "max_coverage_drop": max_coverage_drop,
                        "max_avg_top_score_drop": max_avg_top_score_drop,
                    },
                    "deltas": {
                        "coverage_rate": coverage_delta,
                        "avg_top_score": avg_top_score_delta,
                    },
                    "summary": baseline_summary,
                },
                "history": {
                    "path": history_path.display().to_string(),
                    "window": history_window,
                    "entries_before": history_entries_before.len(),
                    "entries_after": history_entries_after.len(),
                    "previous": history_previous,
                    "delta_vs_previous": history_delta_vs_previous,
                    "window_before_current": window_before,
                    "window_with_current": window_with_current,
                },
                "report_out": report_out_path.as_ref().map(|path| path.display().to_string()),
            });

            if let Some(path) = report_out_path.as_ref() {
                save_json_report(path, &payload)?;
            }

            if fail_on_regression && baseline_regressed {
                return Err(MosaicError::Validation(format!(
                    "knowledge evaluation regressed against baseline at {}: {}",
                    baseline_path.display(),
                    if baseline_reasons.is_empty() {
                        "regression detected".to_string()
                    } else {
                        baseline_reasons.join("; ")
                    }
                )));
            }

            if cli.json {
                print_json(&payload);
            } else {
                println!(
                    "knowledge evaluate: namespace={} top_k={} min_score={}",
                    namespace, top_k, min_score
                );
                println!(
                    "queries: total={} with_hits={} without_hits={} coverage={:.2}%",
                    summary.query_total,
                    summary.with_hits,
                    summary.without_hits,
                    summary.coverage_rate * 100.0
                );
                println!(
                    "avg_hits={:.2} avg_top_score={:.2} p50_top_score={} p90_top_score={}",
                    summary.avg_hits,
                    summary.avg_top_score,
                    summary.p50_top_score,
                    summary.p90_top_score
                );
                for row in rows {
                    println!(
                        "- query=\"{}\" hits={} top_score={} top_paths={}",
                        row.query,
                        row.hit_count,
                        row.top_score,
                        if row.top_paths.is_empty() {
                            "-".to_string()
                        } else {
                            row.top_paths.join(", ")
                        }
                    );
                }
                if let Some(path) = report_out_path.as_ref() {
                    println!("report: {}", path.display());
                }
            }
        }
        KnowledgeCommand::Datasets { command } => match command {
            KnowledgeDatasetsCommand::List { namespace } => {
                let namespace_filter = namespace.as_deref().map(normalize_namespace).transpose()?;
                let records = collect_knowledge_datasets(
                    &paths.root_dir,
                    &paths.data_dir,
                    namespace_filter.as_deref(),
                )?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "count": records.len(),
                        "datasets": records,
                    }));
                } else if records.is_empty() {
                    println!("No knowledge datasets.");
                } else {
                    for record in records {
                        println!(
                            "{} stage={} sources={} staged_files={} memory_docs={} baseline={} history_samples={}",
                            record.namespace,
                            if record.stage_exists { "yes" } else { "no" },
                            if record.stage_sources.is_empty() {
                                "-".to_string()
                            } else {
                                record.stage_sources.join(",")
                            },
                            record.staged_files,
                            record.indexed_documents,
                            if record.baseline_exists { "yes" } else { "no" },
                            record.history_samples,
                        );
                    }
                }
            }
            KnowledgeDatasetsCommand::Remove { namespace, dry_run } => {
                let namespace = normalize_namespace(&namespace)?;
                if namespace == "default" {
                    return Err(MosaicError::Validation(
                        "knowledge datasets remove does not support namespace 'default'"
                            .to_string(),
                    ));
                }
                let result = remove_knowledge_dataset(
                    &paths.root_dir,
                    &paths.data_dir,
                    &namespace,
                    dry_run,
                )?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "result": result,
                    }));
                } else {
                    println!("knowledge dataset remove: {}", result.namespace);
                    println!("dry_run: {}", result.dry_run);
                    println!("stage_root: {}", result.stage_root);
                    println!("removed_stage: {}", result.removed_stage);
                    println!("removed_memory_index: {}", result.removed_memory_index);
                    println!("removed_memory_status: {}", result.removed_memory_status);
                    println!("removed_baseline: {}", result.removed_baseline);
                    println!("removed_history: {}", result.removed_history);
                }
            }
        },
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct StageOptions {
    max_chunk_bytes: usize,
    chunk_overlap_bytes: usize,
    max_file_size: usize,
    max_files: usize,
}

#[derive(Debug, Default)]
struct IngestStats {
    documents_seen: usize,
    documents_skipped: usize,
    chunks_written: usize,
    chunks_reused: usize,
    stale_removed: usize,
}

#[derive(Debug, Clone, Serialize)]
struct HttpIngestEntry {
    url: String,
    ok: bool,
    attempts: u32,
    http_status: Option<u16>,
    bytes: usize,
    skipped: bool,
    error: Option<String>,
}

#[derive(Debug, Default)]
struct HttpIngestStats {
    entries: Vec<HttpIngestEntry>,
    succeeded: usize,
    failed: usize,
}

#[derive(Debug)]
struct HttpFetchSuccess {
    text: String,
    attempts: u32,
    http_status: u16,
}

#[derive(Debug)]
struct HttpFetchFailure {
    attempts: u32,
    http_status: Option<u16>,
    error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KnowledgeEvalQueryResult {
    query: String,
    hit_count: usize,
    top_score: usize,
    top_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KnowledgeEvalSummary {
    query_total: usize,
    with_hits: usize,
    without_hits: usize,
    coverage_rate: f64,
    avg_hits: f64,
    avg_top_score: f64,
    p50_top_score: usize,
    p90_top_score: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KnowledgeEvalBaseline {
    version: u32,
    namespace: String,
    top_k: usize,
    min_score: usize,
    created_unix_ms: i64,
    summary: KnowledgeEvalSummary,
    queries: Vec<KnowledgeEvalQueryResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KnowledgeEvalHistoryEntry {
    version: u32,
    ts_unix_ms: i64,
    namespace: String,
    top_k: usize,
    min_score: usize,
    baseline_regressed: bool,
    summary: KnowledgeEvalSummary,
}

#[derive(Debug, Clone, Serialize)]
struct KnowledgeEvalHistoryWindowStats {
    samples: usize,
    avg_coverage_rate: f64,
    avg_avg_hits: f64,
    avg_avg_top_score: f64,
    avg_p50_top_score: f64,
    avg_p90_top_score: f64,
}

#[derive(Debug, Clone, Serialize)]
struct KnowledgeDatasetRecord {
    namespace: String,
    stage_root: String,
    stage_exists: bool,
    stage_sources: Vec<String>,
    staged_files: usize,
    memory_index_path: String,
    memory_status_path: String,
    memory_exists: bool,
    indexed_documents: usize,
    baseline_path: String,
    baseline_exists: bool,
    history_path: String,
    history_samples: usize,
}

#[derive(Debug, Clone, Serialize)]
struct KnowledgeDatasetRemoveResult {
    namespace: String,
    dry_run: bool,
    stage_root: String,
    removed_stage: bool,
    removed_memory_index: bool,
    removed_memory_status: bool,
    removed_baseline: bool,
    removed_history: bool,
}

fn resolve_source_root(raw: &str) -> Result<PathBuf> {
    let normalized = normalize_non_empty("path", raw)?;
    let path = PathBuf::from(&normalized);
    let root = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|err| MosaicError::Io(err.to_string()))?
            .join(path)
    };
    if !root.exists() {
        return Err(MosaicError::Validation(format!(
            "knowledge source path does not exist: {}",
            root.display()
        )));
    }
    if !root.is_dir() {
        return Err(MosaicError::Validation(format!(
            "knowledge source path must be a directory: {}",
            root.display()
        )));
    }
    Ok(root)
}

fn resolve_mcp_markdown_root(
    data_dir: &Path,
    server_id: &str,
    mcp_path: Option<String>,
) -> Result<PathBuf> {
    let store = McpStore::new(mcp_servers_file_path(data_dir));
    let server = store
        .get(server_id)?
        .ok_or_else(|| MosaicError::Validation(format!("mcp server '{}' not found", server_id)))?;

    let cwd = server.cwd.ok_or_else(|| {
        MosaicError::Validation(format!(
            "mcp server '{}' has no cwd configured; set it with `mosaic mcp add --cwd ...`",
            server_id
        ))
    })?;
    let mut root = PathBuf::from(cwd);
    if root.is_relative() {
        root = std::env::current_dir()
            .map_err(|err| MosaicError::Io(err.to_string()))?
            .join(root);
    }
    if let Some(path) = mcp_path {
        let normalized = normalize_non_empty("mcp_path", &path)?;
        let path = PathBuf::from(normalized);
        root = if path.is_absolute() {
            path
        } else {
            root.join(path)
        };
    }
    if !root.exists() || !root.is_dir() {
        return Err(MosaicError::Validation(format!(
            "mcp source directory not found: {}",
            root.display()
        )));
    }
    Ok(root)
}

fn stage_markdown_from_directory(
    root: &Path,
    stage_root: &Path,
    source_prefix: &str,
    options: &StageOptions,
    expected_files: &mut HashSet<String>,
    stats: &mut IngestStats,
) -> Result<()> {
    let mut processed = 0usize;
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !skip_walk_entry(entry.path()))
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_markdown_file(path) {
            continue;
        }
        if processed >= options.max_files {
            break;
        }
        processed += 1;

        let payload = std::fs::read(path)?;
        if payload.len() > options.max_file_size {
            stats.documents_skipped = stats.documents_skipped.saturating_add(1);
            continue;
        }
        let content = String::from_utf8_lossy(&payload).to_string();
        if content.trim().is_empty() {
            stats.documents_skipped = stats.documents_skipped.saturating_add(1);
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(path);
        let source_id = format!("{source_prefix}://{}", relative.to_string_lossy());
        stage_document(
            stage_root,
            &source_id,
            &content,
            options,
            expected_files,
            stats,
        )?;
    }
    Ok(())
}

async fn stage_markdown_from_http(
    urls: &[String],
    stage_root: &Path,
    timeout_seconds: u64,
    headers: &HeaderMap,
    http_retries: u32,
    http_retry_backoff_ms: u64,
    continue_on_error: bool,
    options: &StageOptions,
    expected_files: &mut HashSet<String>,
    stats: &mut IngestStats,
) -> Result<HttpIngestStats> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()
        .map_err(|err| MosaicError::Network(format!("failed to build HTTP client: {err}")))?;
    let mut http_stats = HttpIngestStats::default();

    for url in urls.iter().take(options.max_files) {
        match fetch_http_markdown(&client, url, headers, http_retries, http_retry_backoff_ms).await
        {
            Ok(fetched) => {
                let bytes = fetched.text.len();
                let mut skipped = false;
                if fetched.text.as_bytes().len() > options.max_file_size {
                    skipped = true;
                    stats.documents_skipped = stats.documents_skipped.saturating_add(1);
                } else {
                    stage_document(
                        stage_root,
                        url,
                        &fetched.text,
                        options,
                        expected_files,
                        stats,
                    )?;
                }
                http_stats.succeeded = http_stats.succeeded.saturating_add(1);
                http_stats.entries.push(HttpIngestEntry {
                    url: url.clone(),
                    ok: true,
                    attempts: fetched.attempts,
                    http_status: Some(fetched.http_status),
                    bytes,
                    skipped,
                    error: None,
                });
            }
            Err(failure) => {
                http_stats.failed = http_stats.failed.saturating_add(1);
                http_stats.entries.push(HttpIngestEntry {
                    url: url.clone(),
                    ok: false,
                    attempts: failure.attempts,
                    http_status: failure.http_status,
                    bytes: 0,
                    skipped: false,
                    error: Some(failure.error.clone()),
                });
                if !continue_on_error {
                    return Err(MosaicError::Network(format!(
                        "failed to fetch '{url}' after {} attempt(s): {} (re-run with --continue-on-error to skip failing URLs)",
                        failure.attempts, failure.error
                    )));
                }
            }
        }
    }
    Ok(http_stats)
}

async fn fetch_http_markdown(
    client: &reqwest::Client,
    url: &str,
    headers: &HeaderMap,
    max_attempts: u32,
    base_backoff_ms: u64,
) -> std::result::Result<HttpFetchSuccess, HttpFetchFailure> {
    let max_attempts = max_attempts.max(1);
    let mut last_error = String::from("unknown error");
    let mut last_status = None;
    for attempt in 1..=max_attempts {
        let response = client.get(url).headers(headers.clone()).send().await;
        match response {
            Ok(response) => {
                let status = response.status();
                last_status = Some(status.as_u16());
                if status.is_success() {
                    let text = response.text().await.map_err(|err| HttpFetchFailure {
                        attempts: attempt,
                        http_status: Some(status.as_u16()),
                        error: format!("failed to read response body: {err}"),
                    })?;
                    return Ok(HttpFetchSuccess {
                        text,
                        attempts: attempt,
                        http_status: status.as_u16(),
                    });
                }

                let body = response.text().await.unwrap_or_default();
                let body_preview = preview_response_body(&body, 240);
                last_error = if body_preview.is_empty() {
                    format!("HTTP {status}")
                } else {
                    format!("HTTP {status}: {body_preview}")
                };
                let retryable = status.is_server_error() || matches!(status.as_u16(), 408 | 429);
                if retryable && attempt < max_attempts {
                    sleep(retry_backoff_duration(base_backoff_ms, attempt)).await;
                    continue;
                }
                return Err(HttpFetchFailure {
                    attempts: attempt,
                    http_status: last_status,
                    error: last_error,
                });
            }
            Err(err) => {
                last_error = err.to_string();
                if attempt < max_attempts {
                    sleep(retry_backoff_duration(base_backoff_ms, attempt)).await;
                    continue;
                }
                return Err(HttpFetchFailure {
                    attempts: attempt,
                    http_status: None,
                    error: last_error,
                });
            }
        }
    }
    Err(HttpFetchFailure {
        attempts: max_attempts,
        http_status: last_status,
        error: last_error,
    })
}

fn save_knowledge_ingest_report(
    path: &Path,
    source: &str,
    namespace: &str,
    source_detail: &serde_json::Value,
    stats: &IngestStats,
    index: &MemoryIndexResult,
    http_stats: Option<&HttpIngestStats>,
    http_retries: u32,
    http_retry_backoff_ms: u64,
) -> Result<()> {
    let mut summary = json!({
        "documents_seen": stats.documents_seen,
        "documents_skipped": stats.documents_skipped,
        "chunks_written": stats.chunks_written,
        "chunks_reused": stats.chunks_reused,
        "stale_removed": stats.stale_removed,
        "indexed_documents": index.indexed_documents,
        "reused_documents": index.reused_documents,
        "reindexed_documents": index.reindexed_documents,
        "removed_documents": index.removed_documents,
    });
    if let Some(value) = http_stats {
        summary["fetched_total"] = json!(value.entries.len());
        summary["fetched_succeeded"] = json!(value.succeeded);
        summary["fetched_failed"] = json!(value.failed);
    }

    let entries = http_stats
        .map(|value| json!(value.entries))
        .unwrap_or_else(|| json!([]));

    let mut payload = json!({
        "ok": true,
        "source": source,
        "namespace": namespace,
        "source_detail": source_detail,
        "summary": summary,
        "index": index,
        "entries": entries,
    });

    if let Some(value) = http_stats {
        payload["retry"] = json!({
            "attempts": http_retries,
            "base_backoff_ms": http_retry_backoff_ms,
        });
        payload["http"] = json!({
            "fetched_total": value.entries.len(),
            "fetched_succeeded": value.succeeded,
            "fetched_failed": value.failed,
        });
    }

    save_json_report(path, &payload)
}

fn save_json_report(path: &Path, payload: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(payload).map_err(|err| {
        MosaicError::Unknown(format!(
            "failed to serialize report '{}': {err}",
            path.display()
        ))
    })?;
    std::fs::write(path, raw)?;
    Ok(())
}

fn default_knowledge_eval_baseline_path(data_dir: &Path, namespace: &str) -> PathBuf {
    data_dir
        .join("knowledge-eval-baselines")
        .join(format!("{namespace}.json"))
}

fn default_knowledge_eval_history_path(data_dir: &Path, namespace: &str) -> PathBuf {
    data_dir
        .join("knowledge-eval-history")
        .join(format!("{namespace}.jsonl"))
}

fn load_knowledge_eval_baseline(path: &Path) -> Result<Option<KnowledgeEvalBaseline>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    let baseline = serde_json::from_str::<KnowledgeEvalBaseline>(&raw).map_err(|err| {
        MosaicError::Validation(format!(
            "invalid knowledge evaluate baseline JSON {}: {err}",
            path.display()
        ))
    })?;
    Ok(Some(baseline))
}

fn save_knowledge_eval_baseline(path: &Path, baseline: &KnowledgeEvalBaseline) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(baseline).map_err(|err| {
        MosaicError::Unknown(format!(
            "failed to serialize knowledge evaluate baseline '{}': {err}",
            path.display()
        ))
    })?;
    std::fs::write(path, raw)?;
    Ok(())
}

fn load_knowledge_eval_history(path: &Path) -> Result<Vec<KnowledgeEvalHistoryEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path)?;
    let mut entries = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry = serde_json::from_str::<KnowledgeEvalHistoryEntry>(line).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid knowledge evaluate history JSONL {} line {}: {err}",
                path.display(),
                index + 1
            ))
        })?;
        entries.push(entry);
    }
    Ok(entries)
}

fn append_knowledge_eval_history(path: &Path, entry: &KnowledgeEvalHistoryEntry) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(entry).map_err(|err| {
        MosaicError::Unknown(format!(
            "failed to serialize knowledge evaluate history '{}': {err}",
            path.display()
        ))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    use std::io::Write as _;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn history_tail(
    entries: &[KnowledgeEvalHistoryEntry],
    window: usize,
) -> &[KnowledgeEvalHistoryEntry] {
    if entries.len() <= window {
        return entries;
    }
    &entries[entries.len().saturating_sub(window)..]
}

fn history_window_stats(entries: &[KnowledgeEvalHistoryEntry]) -> KnowledgeEvalHistoryWindowStats {
    if entries.is_empty() {
        return KnowledgeEvalHistoryWindowStats {
            samples: 0,
            avg_coverage_rate: 0.0,
            avg_avg_hits: 0.0,
            avg_avg_top_score: 0.0,
            avg_p50_top_score: 0.0,
            avg_p90_top_score: 0.0,
        };
    }
    let count = entries.len() as f64;
    KnowledgeEvalHistoryWindowStats {
        samples: entries.len(),
        avg_coverage_rate: entries
            .iter()
            .map(|value| value.summary.coverage_rate)
            .sum::<f64>()
            / count,
        avg_avg_hits: entries
            .iter()
            .map(|value| value.summary.avg_hits)
            .sum::<f64>()
            / count,
        avg_avg_top_score: entries
            .iter()
            .map(|value| value.summary.avg_top_score)
            .sum::<f64>()
            / count,
        avg_p50_top_score: entries
            .iter()
            .map(|value| value.summary.p50_top_score as f64)
            .sum::<f64>()
            / count,
        avg_p90_top_score: entries
            .iter()
            .map(|value| value.summary.p90_top_score as f64)
            .sum::<f64>()
            / count,
    }
}

fn collect_knowledge_datasets(
    root_dir: &Path,
    data_dir: &Path,
    namespace_filter: Option<&str>,
) -> Result<Vec<KnowledgeDatasetRecord>> {
    let sources_root = knowledge_sources_root(root_dir);
    let baseline_root = data_dir.join("knowledge-eval-baselines");
    let history_root = data_dir.join("knowledge-eval-history");

    let mut namespaces = BTreeSet::new();
    if sources_root.exists() {
        for entry in std::fs::read_dir(&sources_root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let raw = entry.file_name().to_string_lossy().to_string();
                let normalized = raw.trim().to_ascii_lowercase();
                if !normalized.is_empty() {
                    namespaces.insert(normalized);
                }
            }
        }
    }

    let memory_statuses = list_memory_namespace_statuses(data_dir)?;
    let mut memory_by_namespace = HashMap::new();
    for status in memory_statuses {
        namespaces.insert(status.namespace.clone());
        memory_by_namespace.insert(status.namespace.clone(), status);
    }

    for namespace in collect_namespace_stems(&baseline_root, "json")? {
        namespaces.insert(namespace);
    }
    for namespace in collect_namespace_stems(&history_root, "jsonl")? {
        namespaces.insert(namespace);
    }

    let mut records = Vec::new();
    for namespace in namespaces {
        if let Some(filter) = namespace_filter
            && namespace != filter
        {
            continue;
        }

        let stage_root = sources_root.join(&namespace);
        let stage_exists = stage_root.exists();
        let mut stage_sources = Vec::new();
        if stage_exists {
            for entry in std::fs::read_dir(&stage_root)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let source = entry.file_name().to_string_lossy().to_string();
                    if !source.trim().is_empty() {
                        stage_sources.push(source);
                    }
                }
            }
            stage_sources.sort();
            stage_sources.dedup();
        }

        let staged_files = if stage_exists {
            count_stage_markdown_files(&stage_root)?
        } else {
            0
        };

        let memory_index_path = memory_index_path_for_namespace(data_dir, &namespace);
        let memory_status_path = memory_status_path_for_namespace(data_dir, &namespace);
        let memory_status = memory_by_namespace.get(&namespace);
        let indexed_documents = memory_status
            .map(|value| value.indexed_documents)
            .unwrap_or(0);
        let memory_exists = memory_status
            .map(|value| value.exists)
            .unwrap_or_else(|| memory_index_path.exists() || memory_status_path.exists());

        let baseline_path = default_knowledge_eval_baseline_path(data_dir, &namespace);
        let history_path = default_knowledge_eval_history_path(data_dir, &namespace);
        let baseline_exists = baseline_path.exists();
        let history_samples = count_jsonl_records(&history_path)?;

        let has_artifact = stage_exists
            || baseline_exists
            || history_samples > 0
            || memory_exists
            || indexed_documents > 0;
        if namespace_filter.is_none() && (namespace == "default" || !has_artifact) {
            continue;
        }

        records.push(KnowledgeDatasetRecord {
            namespace,
            stage_root: stage_root.display().to_string(),
            stage_exists,
            stage_sources,
            staged_files,
            memory_index_path: memory_index_path.display().to_string(),
            memory_status_path: memory_status_path.display().to_string(),
            memory_exists,
            indexed_documents,
            baseline_path: baseline_path.display().to_string(),
            baseline_exists,
            history_path: history_path.display().to_string(),
            history_samples,
        });
    }

    records.sort_by(|lhs, rhs| lhs.namespace.cmp(&rhs.namespace));
    Ok(records)
}

fn remove_knowledge_dataset(
    root_dir: &Path,
    data_dir: &Path,
    namespace: &str,
    dry_run: bool,
) -> Result<KnowledgeDatasetRemoveResult> {
    let stage_root = knowledge_sources_root(root_dir).join(namespace);
    let memory_index_path = memory_index_path_for_namespace(data_dir, namespace);
    let memory_status_path = memory_status_path_for_namespace(data_dir, namespace);
    let baseline_path = default_knowledge_eval_baseline_path(data_dir, namespace);
    let history_path = default_knowledge_eval_history_path(data_dir, namespace);

    let stage_exists = stage_root.exists();
    let memory_index_exists = memory_index_path.exists();
    let memory_status_exists = memory_status_path.exists();
    let baseline_exists = baseline_path.exists();
    let history_exists = history_path.exists();

    if !dry_run {
        if stage_exists {
            std::fs::remove_dir_all(&stage_root)?;
        }
        if memory_index_exists {
            std::fs::remove_file(&memory_index_path)?;
        }
        if memory_status_exists {
            std::fs::remove_file(&memory_status_path)?;
        }
        let memory_namespace_root = data_dir.join("memory/namespaces").join(namespace);
        if memory_namespace_root.exists() && memory_namespace_root.read_dir()?.next().is_none() {
            std::fs::remove_dir(&memory_namespace_root)?;
        }
        if baseline_exists {
            std::fs::remove_file(&baseline_path)?;
        }
        if history_exists {
            std::fs::remove_file(&history_path)?;
        }
    }

    Ok(KnowledgeDatasetRemoveResult {
        namespace: namespace.to_string(),
        dry_run,
        stage_root: stage_root.display().to_string(),
        removed_stage: stage_exists,
        removed_memory_index: memory_index_exists,
        removed_memory_status: memory_status_exists,
        removed_baseline: baseline_exists,
        removed_history: history_exists,
    })
}

fn collect_namespace_stems(root: &Path, extension: &str) -> Result<Vec<String>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some(extension) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let normalized = stem.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            values.push(normalized);
        }
    }
    values.sort();
    values.dedup();
    Ok(values)
}

fn count_stage_markdown_files(root: &Path) -> Result<usize> {
    let mut count = 0usize;
    for entry in WalkDir::new(root).into_iter().flatten() {
        if entry.file_type().is_file() && is_markdown_file(entry.path()) {
            count = count.saturating_add(1);
        }
    }
    Ok(count)
}

fn count_jsonl_records(path: &Path) -> Result<usize> {
    if !path.exists() {
        return Ok(0);
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(raw.lines().filter(|line| !line.trim().is_empty()).count())
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| i64::try_from(value.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn stage_document(
    stage_root: &Path,
    source_id: &str,
    content: &str,
    options: &StageOptions,
    expected_files: &mut HashSet<String>,
    stats: &mut IngestStats,
) -> Result<()> {
    let chunks = split_markdown_chunks(
        content,
        options.max_chunk_bytes,
        options.chunk_overlap_bytes,
    );
    if chunks.is_empty() {
        stats.documents_skipped = stats.documents_skipped.saturating_add(1);
        return Ok(());
    }
    stats.documents_seen = stats.documents_seen.saturating_add(1);
    let doc_key = stable_doc_key(source_id);
    for (index, chunk) in chunks.iter().enumerate() {
        let filename = format!("{doc_key}--{:04}.md", index + 1);
        expected_files.insert(filename.clone());
        let path = stage_root.join(&filename);
        let payload = format!(
            "<!-- source: {source_id} chunk: {}/{} -->\n\n{}",
            index + 1,
            chunks.len(),
            chunk
        );
        if write_if_changed(&path, &payload)? {
            stats.chunks_written = stats.chunks_written.saturating_add(1);
        } else {
            stats.chunks_reused = stats.chunks_reused.saturating_add(1);
        }
    }
    Ok(())
}

fn write_if_changed(path: &Path, content: &str) -> Result<bool> {
    if let Ok(existing) = std::fs::read_to_string(path)
        && existing == content
    {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(true)
}

fn remove_stale_stage_files(stage_root: &Path, expected_files: &HashSet<String>) -> Result<usize> {
    if !stage_root.exists() {
        return Ok(0);
    }
    let mut removed = 0usize;
    for entry in std::fs::read_dir(stage_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.ends_with(".md") {
            continue;
        }
        if !expected_files.contains(&file_name) {
            std::fs::remove_file(entry.path())?;
            removed = removed.saturating_add(1);
        }
    }
    Ok(removed)
}

fn collect_http_urls(urls: Vec<String>, url_file: Option<String>) -> Result<Vec<String>> {
    let mut merged = BTreeSet::new();
    for url in urls {
        let normalized = normalize_non_empty("url", &url)?;
        validate_http_url(&normalized)?;
        merged.insert(normalized);
    }
    if let Some(path) = url_file {
        let path = normalize_non_empty("url_file", &path)?;
        let raw = std::fs::read_to_string(path)?;
        for line in raw.lines() {
            let value = line.trim();
            if value.is_empty() || value.starts_with('#') {
                continue;
            }
            validate_http_url(value)?;
            merged.insert(value.to_string());
        }
    }
    Ok(merged.into_iter().collect())
}

fn collect_evaluation_queries(
    queries: Vec<String>,
    query_file: Option<String>,
) -> Result<Vec<String>> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();
    for raw in queries {
        let query = normalize_non_empty("query", &raw)?;
        if seen.insert(query.clone()) {
            merged.push(query);
        }
    }
    if let Some(path) = query_file {
        let path = normalize_non_empty("query_file", &path)?;
        let raw = std::fs::read_to_string(path)?;
        for line in raw.lines() {
            let value = line.trim();
            if value.is_empty() || value.starts_with('#') {
                continue;
            }
            let query = normalize_non_empty("query", value)?;
            if seen.insert(query.clone()) {
                merged.push(query);
            }
        }
    }
    Ok(merged)
}

fn validate_http_url(url: &str) -> Result<()> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(MosaicError::Validation(format!(
            "invalid http url '{}': only http:// and https:// are supported",
            url
        )));
    }
    Ok(())
}

fn split_markdown_chunks(
    content: &str,
    max_chunk_bytes: usize,
    overlap_bytes: usize,
) -> Vec<String> {
    let normalized = content.trim();
    if normalized.is_empty() {
        return Vec::new();
    }
    let max_chunk_bytes = max_chunk_bytes.max(1_024);
    let overlap_bytes = overlap_bytes.min(max_chunk_bytes / 2);
    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in normalized.lines() {
        let heading_break =
            line.starts_with('#') && !current.is_empty() && current.len() >= max_chunk_bytes / 2;
        let size_break =
            !current.is_empty() && current.len().saturating_add(line.len() + 1) > max_chunk_bytes;
        if heading_break || size_break {
            let finalized = current.trim();
            if !finalized.is_empty() {
                chunks.push(finalized.to_string());
            }
            current = overlap_tail(finalized, overlap_bytes);
            if !current.is_empty() && !current.ends_with('\n') {
                current.push('\n');
            }
        }
        current.push_str(line);
        current.push('\n');
    }

    let tail = current.trim();
    if !tail.is_empty() {
        chunks.push(tail.to_string());
    }
    chunks
}

fn overlap_tail(input: &str, max_bytes: usize) -> String {
    if input.is_empty() || max_bytes == 0 || input.len() <= max_bytes {
        return input.to_string();
    }
    let mut start = input.len().saturating_sub(max_bytes);
    while start < input.len() && !input.is_char_boundary(start) {
        start = start.saturating_add(1);
    }
    input[start..].to_string()
}

fn stable_doc_key(source_id: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in source_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let slug = source_id
        .chars()
        .map(|item| {
            if item.is_ascii_alphanumeric() {
                item.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-");
    format!("{slug}-{hash:016x}")
}

fn knowledge_stage_root(root_dir: &Path, namespace: &str, source: KnowledgeSourceArg) -> PathBuf {
    knowledge_sources_root(root_dir)
        .join(namespace)
        .join(knowledge_source_name(source))
}

fn knowledge_sources_root(root_dir: &Path) -> PathBuf {
    let base = if root_dir.file_name().and_then(|value| value.to_str()) == Some(".mosaic") {
        root_dir
            .parent()
            .unwrap_or(root_dir)
            .join(".mosaic-knowledge")
    } else {
        root_dir.join("knowledge")
    };
    base.join("sources")
}

fn knowledge_source_name(source: KnowledgeSourceArg) -> &'static str {
    match source {
        KnowledgeSourceArg::LocalMd => "local_md",
        KnowledgeSourceArg::Http => "http",
        KnowledgeSourceArg::Mcp => "mcp",
    }
}

fn normalize_namespace(raw: &str) -> Result<String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(MosaicError::Validation(
            "knowledge namespace cannot be empty".to_string(),
        ));
    }
    if value.len() > 64 {
        return Err(MosaicError::Validation(
            "knowledge namespace cannot exceed 64 characters".to_string(),
        ));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return Err(MosaicError::Validation(
            "knowledge namespace only supports [a-zA-Z0-9._-]".to_string(),
        ));
    }
    Ok(value.to_ascii_lowercase())
}

fn ensure_positive_usize(name: &str, value: usize) -> Result<usize> {
    if value == 0 {
        return Err(MosaicError::Validation(format!(
            "{name} must be greater than 0"
        )));
    }
    Ok(value)
}

fn ensure_positive_u64(name: &str, value: u64) -> Result<u64> {
    if value == 0 {
        return Err(MosaicError::Validation(format!(
            "{name} must be greater than 0"
        )));
    }
    Ok(value)
}

fn ensure_positive_u32(name: &str, value: u32) -> Result<u32> {
    if value == 0 {
        return Err(MosaicError::Validation(format!(
            "{name} must be greater than 0"
        )));
    }
    Ok(value)
}

fn ensure_non_negative_overlap(value: usize, max_chunk_bytes: usize) -> Result<usize> {
    if value >= max_chunk_bytes {
        return Err(MosaicError::Validation(
            "chunk_overlap_bytes must be smaller than max_chunk_bytes".to_string(),
        ));
    }
    Ok(value)
}

fn normalize_non_empty(name: &str, value: &str) -> Result<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(MosaicError::Validation(format!("{name} cannot be empty")));
    }
    Ok(normalized.to_string())
}

fn parse_http_headers(
    headers: Vec<String>,
    header_envs: Vec<String>,
) -> Result<(HeaderMap, Vec<String>)> {
    let mut map = HeaderMap::new();
    let mut names = BTreeSet::new();
    for raw in headers {
        let (name, value) = parse_assignment(&raw, "--header")?;
        insert_http_header(&mut map, &mut names, &name, &value)?;
    }
    for raw in header_envs {
        let (name, env_name) = parse_assignment(&raw, "--header-env")?;
        let value = std::env::var(&env_name).map_err(|_| {
            MosaicError::Validation(format!(
                "environment variable '{env_name}' from --header-env is not set"
            ))
        })?;
        if value.trim().is_empty() {
            return Err(MosaicError::Validation(format!(
                "environment variable '{env_name}' from --header-env is empty"
            )));
        }
        insert_http_header(&mut map, &mut names, &name, value.trim())?;
    }
    Ok((map, names.into_iter().collect()))
}

fn parse_assignment(raw: &str, option: &str) -> Result<(String, String)> {
    let Some((left, right)) = raw.split_once('=') else {
        return Err(MosaicError::Validation(format!(
            "invalid {option} value '{raw}', expected KEY=VALUE"
        )));
    };
    let key = left.trim();
    let value = right.trim();
    if key.is_empty() || value.is_empty() {
        return Err(MosaicError::Validation(format!(
            "invalid {option} value '{raw}', expected KEY=VALUE"
        )));
    }
    Ok((key.to_string(), value.to_string()))
}

fn insert_http_header(
    map: &mut HeaderMap,
    names: &mut BTreeSet<String>,
    key: &str,
    value: &str,
) -> Result<()> {
    let name = HeaderName::from_bytes(key.as_bytes()).map_err(|err| {
        MosaicError::Validation(format!("invalid HTTP header name '{key}': {err}"))
    })?;
    let header_value = HeaderValue::from_str(value).map_err(|err| {
        MosaicError::Validation(format!("invalid HTTP header value for '{key}': {err}"))
    })?;
    names.insert(name.as_str().to_string());
    map.insert(name, header_value);
    Ok(())
}

fn preview_response_body(body: &str, max_len: usize) -> String {
    let normalized = body.trim();
    if normalized.is_empty() {
        return String::new();
    }
    if normalized.len() <= max_len {
        return normalized.to_string();
    }
    let mut clipped = normalized.chars().take(max_len).collect::<String>();
    clipped.push_str("...");
    clipped
}

fn retry_backoff_duration(base_ms: u64, attempt: u32) -> Duration {
    let shift = attempt.saturating_sub(1).min(10);
    let factor = 1u64 << shift;
    Duration::from_millis(base_ms.saturating_mul(factor))
}

fn percentile_score(scores: &[usize], percentile: usize) -> usize {
    if scores.is_empty() {
        return 0;
    }
    let mut sorted = scores.to_vec();
    sorted.sort_unstable();
    let rank = ((percentile.max(1).min(100) * sorted.len()) + 99) / 100;
    let index = rank.saturating_sub(1).min(sorted.len().saturating_sub(1));
    sorted[index]
}

fn search_hits_for_namespace(
    data_dir: &Path,
    namespace: &str,
    query: &str,
    limit: usize,
    min_score: usize,
) -> Result<Vec<MemorySearchHit>> {
    let store = MemoryStore::new(
        memory_index_path_for_namespace(data_dir, namespace),
        memory_status_path_for_namespace(data_dir, namespace),
    );
    let result = store.search(query, Some(usize::MAX))?;
    let mut hits = result
        .hits
        .into_iter()
        .filter(|hit| hit.score >= min_score)
        .collect::<Vec<_>>();
    if hits.len() > limit {
        hits.truncate(limit);
    }
    Ok(hits)
}

fn render_augmented_prompt(prompt: &str, hits: &[MemorySearchHit]) -> String {
    if hits.is_empty() {
        return prompt.to_string();
    }
    let mut rendered = String::from(
        "Knowledge snippets from local knowledge base are provided below. Use them when relevant and cite the path.\n\n",
    );
    for (index, hit) in hits.iter().enumerate() {
        rendered.push_str(&format!(
            "[{}] path={} score={}\n{}\n\n",
            index + 1,
            hit.path,
            hit.score,
            hit.snippet
        ));
    }
    rendered.push_str("User question:\n");
    rendered.push_str(prompt);
    rendered
}

fn is_markdown_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "markdown" | "mdx")
    )
}

fn skip_walk_entry(path: &Path) -> bool {
    path.components().any(|component| {
        let text = component.as_os_str().to_string_lossy();
        matches!(
            text.as_ref(),
            ".git" | "node_modules" | "target" | ".pnpm-store" | ".mosaic"
        )
    })
}
