## 0. Workflow process

### When edit BACKLOG file

1. Write sentences short, concise, clear, and accurate.
2. Ensure enough context and meaning to be understood by both humans and other AI Agents.
3. Do not use emojis.
4. Do not overuse bold, italic to highlight, except for keywords, concepts, file names, important names.

### When detect a problem that need to be recorded as an Issue

1. Gather information and discrible the problem. Need to be details and precise.
   Other AI Agents and the author need to understand the context.
2. Call `gh issue create` command to create an issue.
3. Record the issue number to the BACKLOG file. If not create issue, discrible the problem in BACKLOG file 
so other AI Agent can understand the context and take action.

### After finish a task

1. Check if any documentation need to be updated.
2. Update the documentation.
3. Check if any tests need to be updated.
4. Run tests available in `Makefile`.
5. Update `CHANGELOG.md` file.

## 1. Code Guide

### Code style

- Functions: 4-20 lines. Split if longer.
- Files: under 500 lines. Split by responsibility.
- One thing per function, one responsibility per module (SRP).
- Names: specific and unique. Avoid explicit names like `data`, `handler`, `Manager`.
  Prefer names that return <5 grep hits in the codebase.
- Types: explicit. No `any`, no `Dict`, no untyped functions.
- No code duplication. Extract shared logic into a function/module.
- Early returns over nested ifs. Max 2 levels of indentation.
- Exception messages must include the offending value and expected shape.

### Comments

- Keep your own comments. Don't strip them on refactor — they carry
  intent and provenance.
- Write WHY, not WHAT. Skip `// increment counter` above `i++`.
- Docstrings on public functions: intent + one usage example.
- Reference issue numbers / commit SHAs when a line exists because
  of a specific bug or upstream constraint.

### Tests

- Tests run with a single command: `<project-specific>`.
- Every new function gets a test. Bug fixes get a regression test.
- Mock external I/O (API, DB, filesystem) with named fake classes,
  not inline stubs.
- Tests must be F.I.R.S.T: fast, independent, repeatable,
  self-validating, timely.

### Dependencies

- Inject dependencies through constructor/parameter, not global/import.
- Wrap third-party libs behind a thin interface owned by this project.

### Structure

- Follow the framework's convention (Rails, Django, Next.js, etc.).
- Prefer small focused modules over god files.
- Predictable paths: controller/model/view, src/lib/test, etc.

### Formatting

- Use the language default formatter (`cargo fmt`, `gofmt`, `prettier`,
  `black`, `rubocop -A`). Don't discuss style beyond that.

### Logging

- Structured JSON when logging for debugging / observability.
- Plain text only for user-facing CLI output.