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
- [../extensions/markdown-skill-pack.yaml](../extensions/markdown-skill-pack.yaml): extension manifest that registers the markdown skill pack

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
