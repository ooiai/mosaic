use super::*;

impl AgentRuntime {
    pub(crate) fn load_session(
        &self,
        req: &RunRequest,
        profile: &ProviderProfile,
        trace: &mut RunTrace,
    ) -> Result<Option<SessionRecord>> {
        let Some(session_id) = req.session_id.as_deref() else {
            return Ok(None);
        };

        trace.bind_session(session_id.to_owned());

        let mut session = match self.ctx.session_store.load(session_id)? {
            Some(session) => session,
            None => SessionRecord::new(
                session_id,
                session_title_from_input(&req.input),
                profile.name.clone(),
                profile.provider_type.clone(),
                profile.model.clone(),
            ),
        };

        session.set_runtime_binding(
            profile.name.clone(),
            profile.provider_type.clone(),
            profile.model.clone(),
        );
        if let Some(ingress) = req.ingress.as_ref() {
            session.bind_ingress_context(ingress);
        }
        session.set_last_run_id(trace.run_id.clone());

        if session.transcript.is_empty() {
            if let Some(system) = req.system.as_ref() {
                session.append_message(TranscriptRole::System, system.clone(), None);
            }
        }

        self.ctx.session_store.save(&session)?;
        Ok(Some(session))
    }

    pub(crate) fn append_session_message(
        &self,
        session: &mut SessionRecord,
        role: TranscriptRole,
        content: impl Into<String>,
        tool_call_id: Option<String>,
    ) -> Result<()> {
        session.append_message(role, content, tool_call_id);
        self.ctx.session_store.save(session)?;
        Ok(())
    }

    pub(crate) fn session_messages(session: &SessionRecord) -> Vec<Message> {
        session
            .transcript
            .iter()
            .map(|message| Message {
                role: match message.role {
                    TranscriptRole::System => Role::System,
                    TranscriptRole::User => Role::User,
                    TranscriptRole::Assistant => Role::Assistant,
                    TranscriptRole::Tool => Role::Tool,
                },
                content: message.content.clone(),
                tool_call_id: message.tool_call_id.clone(),
                attachments: Vec::new(),
            })
            .collect()
    }

    pub(crate) fn session_context_chars(session: &SessionRecord) -> usize {
        session
            .transcript
            .iter()
            .map(|message| message.content.chars().count())
            .sum()
    }

    pub(crate) fn transcript_fragments(session: &SessionRecord) -> Vec<String> {
        session
            .transcript
            .iter()
            .map(|message| {
                format!(
                    "{}: {}",
                    match message.role {
                        TranscriptRole::System => "system",
                        TranscriptRole::User => "user",
                        TranscriptRole::Assistant => "assistant",
                        TranscriptRole::Tool => "tool",
                    },
                    message.content
                )
            })
            .collect()
    }

    pub(crate) fn extract_session_references(input: &str) -> Vec<String> {
        let mut references = Vec::new();
        let mut remaining = input;
        let prefix = "[[session:";

        while let Some(start) = remaining.find(prefix) {
            let candidate = &remaining[start + prefix.len()..];
            let Some(end) = candidate.find("]]") else {
                break;
            };
            let session_id = candidate[..end].trim();
            if !session_id.is_empty() && !references.iter().any(|existing| existing == session_id) {
                references.push(session_id.to_owned());
            }
            remaining = &candidate[end + 2..];
        }

        references
    }

    pub(crate) fn augment_input_with_reference_context(
        input: &str,
        reference_contexts: &[String],
    ) -> String {
        if reference_contexts.is_empty() {
            return input.to_owned();
        }

        format!(
            "{}\n\nReferenced session context:\n{}",
            input,
            reference_contexts
                .iter()
                .map(|context| format!("- {}", context))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    pub(crate) fn lookup_session_reference_summary(
        &self,
        session_id: &str,
    ) -> Result<Option<String>> {
        if let Some(memory) = self.ctx.memory_store.load_session(session_id)? {
            if let Some(summary) = memory.summary.or(memory.compressed_context) {
                return Ok(Some(summary));
            }
        }

        let Some(session) = self.ctx.session_store.load(session_id)? else {
            return Ok(None);
        };
        let fragments = Self::transcript_fragments(&session);
        if fragments.is_empty() {
            return Ok(None);
        }

        Ok(Some(summarize_fragments(
            &fragments,
            self.ctx.memory_policy.note_char_budget,
        )))
    }

    pub(crate) fn resolve_cross_session_contexts(
        &self,
        mut session: Option<&mut SessionRecord>,
        input: &str,
        trace: &mut RunTrace,
    ) -> Result<Vec<String>> {
        let mut contexts = Vec::new();
        for session_id in Self::extract_session_references(input) {
            let Some(summary) = self.lookup_session_reference_summary(&session_id)? else {
                continue;
            };
            trace.add_memory_read(MemoryReadTrace {
                session_id: session_id.clone(),
                source: "cross_session_reference".to_owned(),
                preview: Self::truncate_preview(&summary, 180),
                tags: vec!["explicit_reference".to_owned()],
            });

            if let Some(session_ref) = session.as_deref_mut() {
                session_ref.record_reference(session_id.clone(), "explicit_session_reference");
                self.ctx.session_store.save(session_ref)?;
            }

            contexts.push(format!("session {} => {}", session_id, summary));
        }

        Ok(contexts)
    }

    pub(crate) fn session_messages_for_provider(
        &self,
        session: &SessionRecord,
        reference_contexts: &[String],
        trace: &mut RunTrace,
    ) -> Result<Vec<Message>> {
        let transcript_messages = Self::session_messages(session);
        let mut reference_messages = reference_contexts
            .iter()
            .map(|context| Message {
                role: Role::System,
                content: format!("Referenced session context:\n{}", context),
                tool_call_id: None,
                attachments: Vec::new(),
            })
            .collect::<Vec<_>>();

        if let Some(summary) = session.memory.latest_summary.as_deref() {
            trace.add_memory_read(MemoryReadTrace {
                session_id: session.id.clone(),
                source: "session_summary".to_owned(),
                preview: Self::truncate_preview(summary, 180),
                tags: vec!["session".to_owned()],
            });
        }

        let compression = compress_fragments(
            &Self::transcript_fragments(session),
            &self.ctx.memory_policy,
        );
        if !compression.compressed {
            let mut messages = transcript_messages;
            let insert_at = if matches!(
                messages.first().map(|message| message.role),
                Some(Role::System)
            ) {
                1
            } else {
                0
            };
            messages.splice(insert_at..insert_at, reference_messages.drain(..));
            return Ok(messages);
        }

        let summary = session
            .memory
            .latest_summary
            .clone()
            .unwrap_or_else(|| compression.summary.clone());
        let mut messages = Vec::new();
        let mut recent_messages = transcript_messages;
        if matches!(
            recent_messages.first().map(|message| message.role),
            Some(Role::System)
        ) {
            messages.push(recent_messages.remove(0));
        }
        messages.push(Message {
            role: Role::System,
            content: format!("Compressed conversation summary:\n{}", summary),
            tool_call_id: None,
            attachments: Vec::new(),
        });
        if let Some(compressed_context) = session.memory.compressed_context.as_deref() {
            trace.add_memory_read(MemoryReadTrace {
                session_id: session.id.clone(),
                source: "compressed_context".to_owned(),
                preview: Self::truncate_preview(compressed_context, 180),
                tags: vec!["compression".to_owned()],
            });
        }
        trace.bind_compression(CompressionTrace {
            original_message_count: compression.original_message_count,
            kept_recent_count: compression.kept_recent_count,
            summary_preview: Self::truncate_preview(&compression.summary, 180),
        });
        messages.extend(reference_messages);
        let recent_start = recent_messages
            .len()
            .saturating_sub(compression.kept_recent_count);
        messages.extend(recent_messages.into_iter().skip(recent_start));
        Ok(messages)
    }

    pub(crate) fn persist_session_memory(
        &self,
        session: &mut SessionRecord,
        trace: &mut RunTrace,
    ) -> Result<()> {
        let fragments = Self::transcript_fragments(session);
        if fragments.is_empty() {
            return Ok(());
        }

        let summary = summarize_fragments(&fragments, self.ctx.memory_policy.summary_char_budget);
        let compression = compress_fragments(&fragments, &self.ctx.memory_policy);
        let compressed_context = compression.compressed.then(|| compression.summary.clone());
        let mut record = self
            .ctx
            .memory_store
            .load_session(&session.id)?
            .unwrap_or_else(|| SessionMemoryRecord::new(session.id.clone()));
        record.set_summary(Some(summary.clone()));
        record.set_compressed_context(compressed_context.clone());
        record.record_entry(
            MemoryEntryKind::Summary,
            summary.clone(),
            vec!["session_summary".to_owned()],
        );
        trace.add_memory_write(MemoryWriteTrace {
            session_id: session.id.clone(),
            kind: "summary".to_owned(),
            preview: Self::truncate_preview(&summary, 180),
            tags: vec!["session".to_owned()],
        });
        if let Some(compressed) = compressed_context.clone() {
            record.record_entry(
                MemoryEntryKind::Compression,
                compressed.clone(),
                vec!["compressed_context".to_owned()],
            );
            trace.add_memory_write(MemoryWriteTrace {
                session_id: session.id.clone(),
                kind: "compression".to_owned(),
                preview: Self::truncate_preview(&compressed, 180),
                tags: vec!["compression".to_owned()],
            });
        }
        for reference in &session.references {
            if !record.related_sessions.contains(&reference.session_id) {
                record.link_session(reference.session_id.clone());
                record.record_entry(
                    MemoryEntryKind::CrossSession,
                    format!("{} ({})", reference.session_id, reference.reason),
                    vec!["cross_session".to_owned()],
                );
                trace.add_memory_write(MemoryWriteTrace {
                    session_id: session.id.clone(),
                    kind: "cross_session".to_owned(),
                    preview: format!("{} ({})", reference.session_id, reference.reason),
                    tags: vec!["cross_session".to_owned()],
                });
            }
        }
        self.ctx.memory_store.save_session(&record)?;

        session.set_memory_state(
            Some(summary),
            compressed_context,
            record.entries.len(),
            compression.compressed,
        );
        self.ctx.session_store.save(session)?;
        Ok(())
    }

    pub(crate) fn rebind_session_profile(
        &self,
        session: &mut SessionRecord,
        profile: &ProviderProfile,
    ) -> Result<()> {
        session.set_runtime_binding(
            profile.name.clone(),
            profile.provider_type.clone(),
            profile.model.clone(),
        );
        self.ctx.session_store.save(session)?;
        Ok(())
    }
}
