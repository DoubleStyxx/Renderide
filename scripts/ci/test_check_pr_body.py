"""Unit tests for check_pr_body.py. Run: python3 -m unittest discover -s scripts/ci -p 'test_check_pr_body.py'"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_pr_body as pr_body  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
TEMPLATE = REPO_ROOT / ".github" / "pull_request_template.md"


FILLED = """## Summary

Docs and a lint.

## Verification evidence

`scripts/ci/test_check_pr_body.py` fails if this section is deleted.
Local run: `python3 -m unittest discover -s scripts/ci -p 'test_check_pr_body.py' -v`.

## Pillars advanced

- reliability: the pr-body check fails a pull request that drops a required section.
- No other pillar changes. Evidence is the unit test above.

## Rendering and parity

- [x] This PR does not change rendering or parity.
"""

MAINTENANCE = """## Verification evidence

The negative tests in `scripts/ci/test_check_pr_body.py` fail if the headings are removed.

## Pillars advanced

None, maintenance only. Documentation and the pull request template. No renderer default,
golden, or frame-gate input changes, so no pillar regresses.
"""


def _event(body: str | None, *, include_pull_request: bool = True) -> str:
    if not include_pull_request:
        return json.dumps({"zen": "none"})
    pull_request: dict[str, object] = {}
    if body is not None:
        pull_request["body"] = body
    else:
        pull_request["body"] = None
    return json.dumps({"pull_request": pull_request})


class LintBody(unittest.TestCase):
    def test_filled_body_passes(self) -> None:
        self.assertEqual(pr_body.lint_pr_body(FILLED), [])

    def test_maintenance_only_passes(self) -> None:
        self.assertEqual(pr_body.lint_pr_body(MAINTENANCE), [])

    def test_crlf_and_bom_pass(self) -> None:
        text = "\ufeff" + FILLED.replace("\n", "\r\n")
        self.assertEqual(pr_body.lint_pr_body(text), [])

    def test_cursor_wrapper_comments_pass(self) -> None:
        wrapped = (
            "<!-- CURSOR_AGENT_PR_BODY_BEGIN -->\n"
            + FILLED
            + "\n<!-- CURSOR_AGENT_PR_BODY_END -->\n"
        )
        self.assertEqual(pr_body.lint_pr_body(wrapped), [])

    def test_missing_verification_fails(self) -> None:
        body = MAINTENANCE.replace("## Verification evidence\n", "")
        errors = pr_body.lint_pr_body(body)
        self.assertTrue(any("## Verification evidence" in err for err in errors))
        self.assertFalse(any("## Pillars advanced" in err and "missing" in err for err in errors))

    def test_missing_pillars_fails(self) -> None:
        body = "## Verification evidence\n\nThe unit test fails without this section.\n"
        errors = pr_body.lint_pr_body(body)
        self.assertTrue(any("missing heading '## Pillars advanced'" in err for err in errors))
        self.assertFalse(any("missing heading '## Verification evidence'" in err for err in errors))

    def test_empty_body_fails_both_headings(self) -> None:
        errors = pr_body.lint_pr_body("")
        self.assertTrue(any("## Verification evidence" in err for err in errors))
        self.assertTrue(any("## Pillars advanced" in err for err in errors))

    def test_whitespace_body_fails(self) -> None:
        errors = pr_body.lint_pr_body("\n\n  \n")
        self.assertGreaterEqual(len(errors), 2)

    def test_placeholder_todo_fails_even_with_other_text(self) -> None:
        body = FILLED.replace(
            "Local run:",
            "TODO(pr-body): replace this line with the verification evidence.\nLocal run:",
        )
        errors = pr_body.lint_pr_body(body)
        self.assertTrue(any("TODO(pr-body):" in err for err in errors))

    def test_placeholder_sentinel_fails(self) -> None:
        body = FILLED + "\n<!-- pr-body-lint:placeholder pillars-advanced -->\n"
        errors = pr_body.lint_pr_body(body)
        self.assertTrue(any("pr-body-lint:placeholder" in err for err in errors))

    def test_empty_section_fails(self) -> None:
        body = (
            "## Verification evidence\n\n"
            "## Pillars advanced\n\n"
            "None, maintenance only. Template wording only; no product change.\n"
        )
        errors = pr_body.lint_pr_body(body)
        self.assertTrue(
            any("## Verification evidence" in err and "no text" in err for err in errors)
        )

    def test_html_comment_only_section_fails(self) -> None:
        body = (
            "## Verification evidence\n\n"
            "<!-- still empty after the comment is ignored -->\n\n"
            "## Pillars advanced\n\n"
            "None, maintenance only, because this is a docs change.\n"
        )
        errors = pr_body.lint_pr_body(body)
        self.assertTrue(
            any("## Verification evidence" in err and "no text" in err for err in errors)
        )

    def test_headings_inside_fences_do_not_count(self) -> None:
        body = (
            "```markdown\n"
            "## Verification evidence\n\n"
            "quoted\n"
            "## Pillars advanced\n\n"
            "quoted\n"
            "```\n"
        )
        errors = pr_body.lint_pr_body(body)
        self.assertTrue(any("missing heading '## Verification evidence'" in err for err in errors))
        self.assertTrue(any("missing heading '## Pillars advanced'" in err for err in errors))

    def test_placeholder_inside_fence_is_ignored(self) -> None:
        body = (
            FILLED
            + "\n```\nTODO(pr-body): this quote is the template, not the section.\n"
            "pr-body-lint:placeholder verification-evidence\n```\n"
        )
        self.assertEqual(pr_body.lint_pr_body(body), [])

    def test_wrong_heading_level_does_not_count(self) -> None:
        body = (
            "### Verification evidence\n\n"
            "not a level-2 heading\n\n"
            "### Pillars advanced\n\n"
            "also not\n"
        )
        errors = pr_body.lint_pr_body(body)
        self.assertTrue(any("missing heading '## Verification evidence'" in err for err in errors))

    def test_duplicate_heading_fails(self) -> None:
        body = FILLED + "\n## Verification evidence\n\nsecond copy\n"
        errors = pr_body.lint_pr_body(body)
        self.assertTrue(any("duplicate heading '## Verification evidence'" in err for err in errors))

    def test_template_file_is_rejected(self) -> None:
        text = TEMPLATE.read_text(encoding="utf-8")
        self.assertIn("## Verification evidence", text)
        self.assertIn("## Pillars advanced", text)
        errors = pr_body.lint_pr_body(text)
        self.assertTrue(any("pr-body-lint:placeholder" in err for err in errors))
        self.assertTrue(any("TODO(pr-body):" in err for err in errors))
        self.assertFalse(any(err.startswith("missing heading") for err in errors))


class EventAndCli(unittest.TestCase):
    def test_event_null_body_fails_lint(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "event.json"
            path.write_text(_event(None), encoding="utf-8")
            code = pr_body.main(["--event", str(path)])
        self.assertEqual(code, 1)

    def test_event_filled_body_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "event.json"
            path.write_text(_event(FILLED), encoding="utf-8")
            code = pr_body.main(["--event", str(path)])
        self.assertEqual(code, 0)

    def test_event_missing_pull_request_is_usage_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "event.json"
            path.write_text(_event("", include_pull_request=False), encoding="utf-8")
            code = pr_body.main(["--event", str(path)])
        self.assertEqual(code, 2)

    def test_event_invalid_json_is_usage_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "event.json"
            path.write_text("{", encoding="utf-8")
            code = pr_body.main(["--event", str(path)])
        self.assertEqual(code, 2)

    def test_body_file_placeholder_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "body.md"
            path.write_text(TEMPLATE.read_text(encoding="utf-8"), encoding="utf-8")
            code = pr_body.main(["--body-file", str(path)])
        self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main()
