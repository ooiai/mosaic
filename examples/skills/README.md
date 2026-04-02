# Skill Examples

These examples show the three current skill source types:

- native skill
- manifest skill
- markdown skill pack

Use them together with [docs/skills.md](../../docs/skills.md).

## Native Skill

- [native-skill.yaml](./native-skill.yaml): references the builtin native `summarize` skill

## Manifest Skill

- [manifest-skill.yaml](./manifest-skill.yaml): declarative manifest skill with sequential steps

## Markdown Skill Pack

- [operator-note/SKILL.md](./operator-note/SKILL.md): directory-based markdown skill pack
- [operator-note/templates/note.md](./operator-note/templates/note.md): pack template used during render
- [operator-note/references/escalation.md](./operator-note/references/escalation.md): pack reference material
- [operator-note/scripts/annotate.py](./operator-note/scripts/annotate.py): helper script executed inside the selected sandbox env
- [../extensions/markdown-skill-pack.yaml](../extensions/markdown-skill-pack.yaml): extension manifest that registers the markdown skill pack

The `operator-note` example now demonstrates the full markdown-pack execution path:

- `SKILL.md` frontmatter
- `templates/`
- `references/`
- `scripts/`
- workspace-local sandbox execution

## Validation

Validate skill examples through config or extension loading:

```bash
mosaic setup validate
mosaic extension validate
```

Then verify provenance:

```bash
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```

For local chat-first validation, start the TUI and use slash completion:

```bash
mosaic tui
# then type: /skill op<Tab>
```
