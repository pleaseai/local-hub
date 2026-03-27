# Project Workflow

> Development workflow conventions for local-hub.

## Guiding Principles

1. **The Plan is the Source of Truth**: All work is tracked in the track's `plan.md`
2. **The Tech Stack is Deliberate**: Changes to the tech stack must be documented in `tech-stack.md` before implementation
3. **Test-Driven Development**: Write tests before implementing functionality
4. **High Code Coverage**: Aim for >80% code coverage for new code
5. **Non-Interactive & CI-Aware**: Prefer non-interactive commands. Use `CI=true` for watch-mode tools

## Task Workflow

All tasks follow a strict lifecycle within `/please:implement`:

### Standard Task Lifecycle

1. **Select Task**: Choose the next available task from `plan.md`
2. **Mark In Progress**: Update task status from `[ ]` to `[~]`
3. **Write Failing Tests (Red Phase)**:
   - Create test file for the feature or bug fix
   - Write unit tests defining expected behavior
   - Run tests and confirm they fail as expected
4. **Implement to Pass Tests (Green Phase)**:
   - Write minimum code to make failing tests pass
   - Run test suite and confirm all tests pass
5. **Refactor (Optional)**:
   - Improve clarity, remove duplication, enhance performance
   - Rerun tests to ensure they still pass
6. **Verify Coverage**: Run coverage reports. Target: >80% for new code
7. **Document Deviations**: If implementation differs from tech stack, update `tech-stack.md` first
8. **Commit**: Stage and commit with conventional commit message
9. **Update Progress**: Mark the task as completed in `## Progress` with a timestamp

### Phase Completion Protocol

Executed when all tasks in a phase are complete:

1. **Verify Test Coverage**: Identify all files changed in the phase, ensure test coverage
2. **Run Full Test Suite**: Execute all tests, debug failures (max 2 fix attempts)
3. **Manual Verification Plan**: Generate step-by-step verification instructions for the user
4. **User Confirmation**: Wait for explicit user approval before proceeding
5. **Create Checkpoint**: Commit with message `chore(checkpoint): complete phase {name}`
6. **Update Plan**: Mark phase as complete in `plan.md`

## Quality Gates

Before marking any task complete:

- [ ] All tests pass
- [ ] Code coverage meets requirements (>80%)
- [ ] Code follows project style guidelines
- [ ] No linting or static analysis errors (`cargo clippy`)
- [ ] No security vulnerabilities introduced
- [ ] Documentation updated if needed

## Development Commands

### Setup

```bash
mise install                     # Install Rust toolchain
cargo build                      # Build all crates
```

### Daily Development

```bash
cargo run -p local-hub -- start  # Run the proxy server
cargo watch -x test              # Watch mode testing (requires cargo-watch)
```

### Testing

```bash
cargo test                       # Run all tests
cargo test -p local-hub          # Test server crate only
cargo llvm-cov                   # Coverage report (requires cargo-llvm-cov)
```

### Before Committing

```bash
cargo clippy -- -D warnings      # Lint (treat warnings as errors)
cargo fmt --check                # Format check
cargo test                       # Full test suite
```

## Testing Requirements

### Unit Testing

- Every module must have corresponding tests
- Mock external dependencies (GitHub API responses)
- Test both success and failure cases

### Integration Testing

- Test complete request flows (cache hit, miss, ETag refresh)
- Verify cache invalidation on write requests
- Test Unix socket + TCP dual listen

## Commit Guidelines

Follow conventional commits.

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Formatting changes
- `refactor`: Code change without behavior change
- `test`: Adding or updating tests
- `chore`: Maintenance tasks
- `ci`: CI/CD changes

## Definition of Done

A task is complete when:

1. All code implemented to specification
2. Unit tests written and passing
3. Code coverage meets project requirements
4. `cargo clippy` and `cargo fmt --check` pass
5. Progress updated in `plan.md`
6. Changes committed with proper message
