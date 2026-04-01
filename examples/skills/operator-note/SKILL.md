---
name: operator_note
description: Render an operator note from a markdown skill pack.
version: 0.1.0
allowed_tools:
  - read_file
allowed_channels:
  - telegram
invocation_mode: explicit_only
accepts_attachments: true
runtime_requirements:
  - python
---
Operator note:
{{input}}
