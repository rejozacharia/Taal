# Contribution Guidelines

## Scope
These guidelines apply to the entire repository until more specific instructions are added in nested directories.

## Documentation Style
- Prefer Markdown files stored under `docs/` for architectural plans and specifications.
- Use descriptive headings and ordered lists to outline development roadmaps.
- When adding diagrams or tables, ensure they have accompanying text descriptions for accessibility.

## Code Style Preview
- The core application will be implemented in Rust.
- Structure Rust code using Cargo workspaces with separate crates for the transcription engine, tutoring UI, shared models, and utilities.
- Favor type-safe APIs and exhaustive pattern matching to handle domain-specific drum events.

## Commit Expectations
- Keep commits scoped to a logical change and provide clear messages summarizing the intent.
- Update relevant documentation when altering architecture or workflows.
