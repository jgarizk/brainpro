"""Test 16: Code review (code review, security review, custom command)."""

import pytest

from harness.runner import BrainproRunner
from harness.fixtures import MockWebapp
from harness.assertions import assert_output_contains_any

# Path to mock_webapp_scratch (now in /tmp)
WEBAPP = "/tmp/brainpro-mock-webapp-scratch"


class TestReview:
    """Code review tests."""

    def test_code_review(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to review code for quality issues."""
        prompt = (
            f"Review {WEBAPP} for code quality issues. Look for TODO comments, "
            "deprecated functions, missing documentation, and code smells. "
            "Summarize your findings."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert brainpro found some issues
        assert_output_contains_any(
            result.output,
            "TODO",
            "deprecated",
            "undocumented",
            "documentation",
            "old_query",
        )

    def test_security_review(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to do a security review of services/."""
        prompt = (
            f"Do a security review of all files in {WEBAPP}/src/services/. Look for hardcoded "
            "secrets, credentials, SQL injection risks, and other security issues. "
            "Report what you find."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert brainpro found the security issue
        assert_output_contains_any(
            result.output,
            "hardcoded",
            "api_key",
            "secret",
            "sk-secret",
            "credential",
            "security",
        )

    def test_custom_command(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Use the custom /review command."""
        prompt = (
            f"Read {WEBAPP}/.claude/commands/review.md and follow its instructions to review "
            "the codebase."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert brainpro did some kind of review based on the command
        assert_output_contains_any(
            result.output, "security", "deprecated", "documentation", "TODO", "review"
        )
