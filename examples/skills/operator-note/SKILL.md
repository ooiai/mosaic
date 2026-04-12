---
name: operator_note
description: Render an operator note from a markdown skill pack with templates, references, and a helper script.
version: 0.2.0
template: note.md
references:
  - escalation.md
script: annotate.py
script_runtime: python
allowed_tools:
  - read_file
allowed_channels:
  - telegram
invocation_mode: explicit_only
accepts_attachments: true
runtime_requirements:
  - python
---
Reference material:
{{references.escalation}}

The template and helper script build the final operator-facing note.
