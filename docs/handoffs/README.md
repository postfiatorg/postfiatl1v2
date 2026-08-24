# Handoff Standard

Use handoffs to transfer active context between operators and agents. Keep each
handoff concise, plain-English, and grounded in repository references.

## Filename

`YYYY-MM-DD___operator__simple_title.md`

- Use the UTC handoff date.
- Use a lowercase operator identifier.
- Use a short lowercase `snake_case` title.

## Required format

```markdown
# Simple title

- **Operator:** Name (`operator_id`)
- **Date:** YYYY-MM-DD UTC

## BLUF

One short paragraph stating what was worked on, the current result, and the
primary document references.

## Current state

Concise facts about completed and active work, with code or evidence links.

## Next decision or action

The decision to make or the next bounded action, including any constraints.

## References

- Documents, code, tests, and evidence needed by the next reader.
```

Do not paste logs, speculate about unverified behavior, or describe shadow and
simulation evidence as live authority.
