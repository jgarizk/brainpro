"""Test 13: Documentation (add docs, update readme, find undocumented)."""

import subprocess

import pytest

from harness.runner import BrainproRunner
from harness.fixtures import MockWebapp
from harness.assertions import (
    assert_output_contains_any,
    assert_file_contains,
    assert_git_dirty,
    assert_success,
)

# Path to mock_webapp_scratch (now in /tmp)
WEBAPP = "/tmp/brainpro-mock-webapp-scratch"


class TestDocumentation:
    """Documentation tests."""

    def test_add_docs(self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp):
        """Ask brainpro to add doc comments to undocumented functions."""
        handlers_file = mock_webapp.path / "src" / "api" / "handlers.rs"

        # Check if already documented (skip if so)
        content = handlers_file.read_text()
        lines = content.split("\n")
        for i, line in enumerate(lines):
            if "pub fn get_user" in line and i > 0:
                if "///" in lines[i - 1]:
                    pytest.skip("get_user already has docs")

        prompt = (
            f"Add doc comments (///) to all undocumented public functions in "
            f"{WEBAPP}/src/api/handlers.rs. Each function should have a brief description "
            "of what it does."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert handlers.rs was modified
        assert_git_dirty(mock_webapp.path)

        # Assert file now contains doc comments
        assert_file_contains(handlers_file, "/// ")

        # Verify the project still compiles
        build_result = subprocess.run(
            ["cargo", "build"],
            capture_output=True,
            cwd=mock_webapp.path,
        )
        assert_success(build_result.returncode)

    def test_update_readme(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to add API documentation to README."""
        readme_file = mock_webapp.path / "README.md"
        content = readme_file.read_text()

        # Check if API section exists (skip if so)
        if "## API" in content.upper():
            pytest.skip("README already has API section")

        prompt = (
            f"The {WEBAPP}/README.md is missing API documentation. Add a section documenting "
            f"the available API endpoints/handlers based on what's in {WEBAPP}/src/api/handlers.rs."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert README was modified
        assert_git_dirty(mock_webapp.path)

        # Assert README now has API documentation
        assert_file_contains(readme_file, "API")

    def test_find_undocumented(
        self, webapp_runner: BrainproRunner, mock_webapp: MockWebapp
    ):
        """Ask brainpro to find undocumented functions."""
        prompt = (
            f"Find functions in {WEBAPP}/src/api/ that don't have doc comments (/// comments). "
            "List them."
        )

        result = webapp_runner.oneshot(prompt)

        # Assert brainpro found the undocumented handlers
        assert_output_contains_any(
            result.output,
            "get_user",
            "create_user",
            "login",
            "list_users",
            "delete_user",
            "handlers.rs",
        )
