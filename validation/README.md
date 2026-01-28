# brainpro Manual Validation Framework

A comprehensive validation suite for testing `brainpro` functionality using Python/pytest.

## Overview

This framework validates `brainpro` functionality by testing **outcomes** rather than exact LLM outputs. Since LLM responses are non-deterministic, tests focus on:

- File existence and content (did the expected file get created?)
- Tool invocation (was the right tool called?)
- Exit codes and error handling
- Semantic content (does the output contain expected keywords?)

## Prerequisites

1. **Build brainpro**:
   ```bash
   cargo build --release
   ```

2. **Set API key** (Venice is default):
   ```bash
   export VENICE_API_KEY="your-key-here"
   # Or: export ANTHROPIC_API_KEY="..."
   # Or: export OPENAI_API_KEY="..."
   ```

3. **Install Python dependencies**:
   ```bash
   cd validation
   pip install -r requirements.txt
   ```

## Running Tests

### Run All Tests (yo mode - default)
```bash
cd validation
pytest
# Or:
./run_tests.py
```

### Run with Different Modes
```bash
# Direct yo binary (default)
pytest --mode=yo

# Native gateway (starts brainpro-gateway + brainpro-agent)
pytest --mode=native

# Docker gateway (uses docker-compose)
pytest --mode=docker

# Or use environment variable
BRAINPRO_TEST_MODE=docker pytest
```

### Run Specific Category
```bash
pytest tests/test_01_tools.py
pytest tests/test_05_agent_loop.py
pytest tests/test_06_plan_mode.py
```

### Run Single Test
```bash
pytest tests/test_01_tools.py::TestTools::test_read_basic
pytest -k test_read_basic
```

### Useful Options
```bash
pytest -v                      # Verbose output
pytest -vv                     # More verbose
pytest -s                      # Show stdout/stderr
pytest -x                      # Stop on first failure
pytest -k "tools or loop"      # Run tests matching pattern
```

## Test Categories

| Category | Description | Tests | Est. Cost |
|----------|-------------|-------|-----------|
| 01-tools | Basic tool operations | 7 | ~$0.06 |
| 02-exploration | Codebase understanding | 3 | ~$0.03 |
| 03-editing | File creation/modification | 2 | ~$0.04 |
| 04-build | cargo build/test | 2 | ~$0.03 |
| **05-agent-loop** | **CORE: Multi-turn REPL** | 4 | ~$0.15 |
| **06-plan-mode** | **CORE: Plan workflow** | 4 | ~$0.12 |
| 07-subagents | Task delegation | 3 | ~$0.10 |
| 08-permissions | Policy enforcement | 2 | ~$0.02 |
| 09-errors | Error handling | 2 | ~$0.02 |
| 10-tdd | Test-driven development | 3 | ~$0.10 |
| 11-debugging | Bug fixing & security review | 3 | ~$0.08 |
| 12-refactoring | Code modernization | 3 | ~$0.08 |
| 13-documentation | Doc generation | 3 | ~$0.08 |
| 14-git | Git operations | 3 | ~$0.08 |
| 15-multi-file | Cross-file changes | 3 | ~$0.12 |
| 16-review | Code review | 3 | ~$0.08 |
| 18-new-tools | TodoWrite, Session, PlanMode | 3 | ~$0.08 |

**Total estimated cost: ~$1.25 per full run** (52 tests)

## Test Priority

For quick validation, run in this order:

1. `test_01_tools` - Verify basic tool operations work
2. `test_05_agent_loop` - Validate multi-turn conversations (CORE)
3. `test_06_plan_mode` - Validate plan mode workflow (CORE)

## Execution Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `yo` | Direct `target/release/yo` binary | Default, simplest |
| `native` | Harness starts gateway + agent processes | Test gateway mode locally |
| `docker` | Harness uses docker-compose | Full containerized test |

All 52 tests run in all modes. The `yo` binary is used in all modes; gateway modes pass `--gateway URL`.

## Writing New Tests

### Test Structure
```python
"""Test description."""

import pytest
from harness.runner import BrainproRunner
from harness.assertions import (
    assert_exit_code,
    assert_output_contains,
    assert_file_exists,
    assert_tool_called,
)

class TestCategory:
    """Category description."""

    def test_feature(self, runner: BrainproRunner, scratch_dir):
        """Test description."""
        prompt = "Your prompt here"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_contains("expected", result.output)
        assert_file_exists(scratch_dir.path / "file.txt")
        assert_tool_called("Read", result.output)
```

### Available Fixtures

**From conftest.py:**
- `runner` - `BrainproRunner` instance for running commands
- `scratch_dir` - Clean scratch directory (`fixtures/scratch`)
- `mock_webapp` - Fresh copy of mock_webapp
- `webapp_runner` - Runner that operates in mock_webapp directory
- `sessions_dir` - Session directory manager
- `fixtures_dir` - Path to fixtures directory
- `hello_repo` - Path to hello_repo fixture

### Available Assertions

**Exit codes:**
- `assert_exit_code(expected, actual)`
- `assert_success(exit_code)`
- `assert_failure(exit_code)`

**Output assertions:**
- `assert_output_contains(needle, output)` - Case-insensitive
- `assert_output_not_contains(needle, output)`
- `assert_output_matches(pattern, output)` - Regex
- `assert_output_contains_any(output, *patterns)`
- `assert_equals(expected, actual)`

**File assertions:**
- `assert_file_exists(path)`
- `assert_file_not_exists(path)`
- `assert_file_contains(path, needle)`
- `assert_file_not_contains(path, needle)`
- `assert_dir_exists(path)`

**Tool assertions:**
- `assert_tool_called(tool_name, output)`
- `assert_tools_called(output, *tools)`

**Cargo assertions:**
- `assert_cargo_test_passes(project_dir)`
- `assert_cargo_test_fails(project_dir)`
- `assert_single_test_passes(project_dir, test_name)`
- `assert_single_test_fails(project_dir, test_name)`

**Git assertions:**
- `assert_git_clean(repo_dir)`
- `assert_git_dirty(repo_dir)`
- `assert_git_has_commits(repo_dir, min_count)`

### Runner Methods

```python
# One-shot mode (single prompt)
result = runner.oneshot("prompt", "--mode", "acceptEdits")

# REPL mode (multiple commands)
result = runner.repl(
    "first command",
    "second command",
    "/exit"
)

# Without --yes flag (for permission tests)
result = runner.oneshot_no_yes("prompt", stdin_input="n")

# Run in specific directory
webapp_runner = runner.with_working_dir(path)
```

## Results

Pytest generates standard output. For detailed logs, use:
```bash
pytest -v --tb=long
pytest -s  # Show all output
```

## Design Principles

1. **Bound inputs tightly**: Use specific fixtures and prompts
2. **Bound outcomes loosely**: Check for presence, not exact match
3. **Case-insensitive**: LLM capitalization varies
4. **Multiple valid patterns**: Use regex alternation `(pass|ok|success)`
5. **Idempotent**: Each test resets its scratch state

## Fixtures

- `fixtures/hello_repo/` - Simple Rust project for basic tests
- `fixtures/mock_webapp/` - Full web app with intentional issues:
  - Security issue: hardcoded API key in `auth.rs`
  - Deprecated function: `old_query()` in `database.rs`
  - Validation bug: `@.` passes email validation
  - Undocumented functions in `handlers.rs`
  - One intentionally failing test
- `/tmp/brainpro-mock-webapp-scratch/` - Ephemeral copy for tests that mutate
- `/tmp/brainpro-test-scratch/` - Ephemeral directory for test artifacts
- `fixtures/agents/` - Subagent configurations

## Troubleshooting

### Test fails with "yo binary not found"
```bash
cargo build --release
```

### Test fails with "No API key"
```bash
export VENICE_API_KEY="your-key"
```

### Test times out
Some tests may take 30-60 seconds due to LLM response time.

### Test is flaky
LLM outputs are non-deterministic. If a test fails:
1. Run with `-v -s` to see output
2. Consider loosening assertions (add more valid patterns)
3. Ensure the prompt is specific enough

### Gateway mode fails to start
For native mode, ensure gateway binaries are built:
```bash
cargo build --release
```

For docker mode, ensure Docker is running:
```bash
docker compose up -d --build
```
