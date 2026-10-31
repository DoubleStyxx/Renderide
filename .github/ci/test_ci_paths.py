"""Unit tests for ci_paths.py."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_paths  # noqa: E402


ROOT = Path(__file__).resolve().parents[2]


class PatternTests(unittest.TestCase):
    def test_historical_suite_paths_match(self) -> None:
        samples = [
            ".github/workflows/ci.yml",
            ".github/actions/lavapipe/action.yml",
            ".github/ci/ci_paths.py",
            ".github/ci/required_check.py",
            ".github/ci/macos_capacity_note.py",
            ".taplo.toml",
            "Cargo.lock",
            "Cargo.toml",
            "Generators.sln",
            "clippy.toml",
            "crates/renderide/src/lib.rs",
            "crates/renderide-test/src/scene_dsl.rs",
            "generators/SharedTypeGenerator/Program.cs",
            "third_party/example.c",
            "RenderideMod/Shaders/foo.wgsl",
        ]
        for path in samples:
            with self.subTest(path=path):
                self.assertTrue(ci_paths.path_selects_suite(path))

    def test_docs_codeql_and_bench_do_not_match(self) -> None:
        samples = [
            "README.md",
            "CONTRIBUTING.md",
            "docs/ci/README.md",
            "docs/ci/master-ruleset.json",
            "docs/parity-audit.md",
            ".github/workflows/codeql.yml",
            ".github/workflows/bench-pr.yml",
            ".github/workflows/frame-bench.yml",
            ".github/workflows/nightly.yml",
            "RenderideMod/RenderideMod.csproj",
            "RenderideMod/Shaders",
            "crates",
            "not-crates/lib.rs",
            "crates-extra/lib.rs",
            "Cargo.toml.bak",
        ]
        for path in samples:
            with self.subTest(path=path):
                self.assertFalse(ci_paths.path_selects_suite(path))

    def test_unsupported_pattern_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            ci_paths.match_pattern("a.rs", "*.rs")

    def test_name_status_includes_rename_endpoints(self) -> None:
        blob = b"M\0README.md\0R100\0crates/old.rs\0docs/new.rs\0A\0generators/A.cs\0"
        self.assertEqual(
            ci_paths.paths_from_name_status_z(blob),
            ["README.md", "crates/old.rs", "docs/new.rs", "generators/A.cs"],
        )
        self.assertTrue(ci_paths.diff_selects_suites(ci_paths.paths_from_name_status_z(blob)))

    def test_docs_only_name_status_does_not_select(self) -> None:
        blob = b"M\0docs/ci/README.md\0A\0README.md\0"
        paths = ci_paths.paths_from_name_status_z(blob)
        self.assertFalse(ci_paths.diff_selects_suites(paths))

    def test_truncated_record_is_unavailable(self) -> None:
        with self.assertRaises(ci_paths.DiffUnavailable):
            ci_paths.paths_from_name_status_z(b"R100\0only-old.rs")

    def test_empty_diff(self) -> None:
        self.assertEqual(ci_paths.paths_from_name_status_z(b""), [])
        self.assertFalse(ci_paths.diff_selects_suites([]))


class DecisionTests(unittest.TestCase):
    def test_pull_request_docs_only_skips(self) -> None:
        self.assertEqual(ci_paths.suites_output("pull_request", ["docs/ci/README.md"]), "false")

    def test_push_of_crate_runs(self) -> None:
        self.assertEqual(ci_paths.suites_output("push", ["crates/renderide/src/lib.rs"]), "true")

    def test_unknown_diff_runs_suites(self) -> None:
        self.assertEqual(ci_paths.suites_output("pull_request", None), "true")
        self.assertEqual(ci_paths.suites_output("workflow_dispatch", None), "true")
        self.assertEqual(ci_paths.suites_output("schedule", ["README.md"]), "true")

    def test_non_sha_is_rejected(self) -> None:
        with self.assertRaises(ci_paths.DiffUnavailable):
            ci_paths.git_diff("master", "HEAD")
        with self.assertRaises(ci_paths.DiffUnavailable):
            ci_paths.validate_sha("0" * 40)

    def test_zero_before_on_push_runs_suites(self) -> None:
        changed = ci_paths.changed_files(
            "push",
            head="a" * 40,
            base="",
            before="0" * 40,
        )
        self.assertIsNone(changed)
        suites, paths, _reason = ci_paths.decide(
            "push",
            head="a" * 40,
            base="",
            before="0" * 40,
        )
        self.assertEqual(suites, "true")
        self.assertIsNone(paths)

    def test_workflow_dispatch_runs_suites(self) -> None:
        suites, paths, _reason = ci_paths.decide(
            "workflow_dispatch",
            head="",
            base="",
            before="",
        )
        self.assertEqual(suites, "true")
        self.assertIsNone(paths)


class GitDiffTests(unittest.TestCase):
    def test_rename_out_of_crates_still_selects_suites(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._git(root, "init")
            self._git(root, "config", "user.email", "ci@example.com")
            self._git(root, "config", "user.name", "ci")
            crate = root / "crates"
            crate.mkdir()
            (crate / "a.rs").write_text("fn a() {}\n", encoding="utf-8")
            (root / "README.md").write_text("docs\n", encoding="utf-8")
            self._git(root, "add", ".")
            self._git(root, "commit", "-m", "base")
            base = self._rev(root)
            docs = root / "docs"
            docs.mkdir()
            (crate / "a.rs").rename(docs / "a.rs")
            self._git(root, "add", "-A")
            self._git(root, "commit", "-m", "move")
            head = self._rev(root)
            paths = ci_paths.git_diff(base, head, cwd=str(root))
            self.assertIn("crates/a.rs", paths)
            self.assertIn("docs/a.rs", paths)
            self.assertTrue(ci_paths.diff_selects_suites(paths))

            (root / "README.md").write_text("docs only\n", encoding="utf-8")
            self._git(root, "add", ".")
            self._git(root, "commit", "-m", "docs")
            docs_head = self._rev(root)
            docs_paths = ci_paths.git_diff(head, docs_head, cwd=str(root))
            self.assertEqual(docs_paths, ["README.md"])
            suites, _paths, _reason = ci_paths.decide(
                "pull_request",
                head=docs_head,
                base=head,
                before="",
                cwd=str(root),
            )
            self.assertEqual(suites, "false")

    def _git(self, root: Path, *args: str) -> None:
        subprocess.run(["git", *args], cwd=root, check=True, capture_output=True, text=True)

    def _rev(self, root: Path) -> str:
        completed = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        )
        return completed.stdout.strip()


class RulesetTests(unittest.TestCase):
    def test_master_ruleset_requires_only_the_aggregate(self) -> None:
        import json

        payload = json.loads((ROOT / "docs/ci/master-ruleset.json").read_text(encoding="utf-8"))
        self.assertEqual(payload["target"], "branch")
        self.assertEqual(payload["enforcement"], "active")
        self.assertEqual(payload["bypass_actors"], [])
        self.assertEqual(payload["conditions"]["ref_name"]["include"], ["~DEFAULT_BRANCH"])
        self.assertEqual(payload["conditions"]["ref_name"]["exclude"], [])
        rules = {rule["type"]: rule for rule in payload["rules"]}
        self.assertEqual(set(rules), {"deletion", "non_fast_forward", "pull_request", "required_status_checks"})
        pull = rules["pull_request"]["parameters"]
        self.assertEqual(pull["required_approving_review_count"], 0)
        self.assertTrue(pull["dismiss_stale_reviews_on_push"])
        checks = rules["required_status_checks"]["parameters"]
        self.assertTrue(checks["strict_required_status_checks_policy"])
        self.assertEqual(
            checks["required_status_checks"],
            [{"context": "required", "integration_id": 15368}],
        )


class WorkflowContractTests(unittest.TestCase):
    def test_trigger_has_no_path_filter_and_required_is_wired(self) -> None:
        text = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertNotIn("\n    paths:", text)
        self.assertIn("needs: [changes, rust]", text)
        self.assertIn("needs: changes\n", text)
        self.assertIn("if: needs.changes.outputs.suites == 'true'", text)
        self.assertIn("if: always() && needs.changes.outputs.suites == 'true'", text)
        self.assertIn("if: always()\n", text)
        self.assertIn("name: required", text)
        self.assertIn("name: Rust / ${{ matrix.os }}", text)
        self.assertIn("name: .NET / ${{ matrix.os }}", text)
        self.assertIn("name: macOS capacity", text)
        self.assertIn("check-suite", text)
        self.assertIn("./.github/actions/lavapipe", text)
        self.assertIn("CHANGES_RESULT:", text)
        self.assertIn("actions: read", text)
        for line in (
            "        run: python3 -m unittest discover -s .github/ci -p 'test_*.py' -v",
            "        run: python3 .github/ci/ci_paths.py",
            "        run: python3 .github/ci/required_check.py --wait",
        ):
            self.assertIn(line, text)
            self.assertNotIn("${{", line)


if __name__ == "__main__":
    unittest.main()
