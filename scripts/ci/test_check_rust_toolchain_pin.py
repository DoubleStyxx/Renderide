"""Unit tests for check_rust_toolchain_pin.py. Run: python3 -m unittest discover -s scripts/ci -p 'test_*.py'"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_rust_toolchain_pin as pin  # noqa: E402


PINNED = "1.99.1"


def write_tree(root: Path, toml: str, workflows: dict[str, str]) -> None:
    (root / "rust-toolchain.toml").write_text(toml, encoding="utf-8")
    wf = root / ".github" / "workflows"
    wf.mkdir(parents=True)
    for name, body in workflows.items():
        (wf / name).write_text(body, encoding="utf-8")


class ParsePin(unittest.TestCase):
    def test_exact_version(self) -> None:
        text = '[toolchain]\nchannel = "1.99.1"\ncomponents = ["rustfmt", "clippy"]\n'
        self.assertEqual(pin.parse_pin(text), "1.99.1")

    def test_rejects_stable(self) -> None:
        with self.assertRaises(ValueError):
            pin.parse_pin('[toolchain]\nchannel = "stable"\n')

    def test_rejects_missing(self) -> None:
        with self.assertRaises(ValueError):
            pin.parse_pin("[toolchain]\ncomponents = [\"rustfmt\"]\n")


class CheckFile(unittest.TestCase):
    def test_ok_pinned_action_sha_and_version(self) -> None:
        text = (
            "jobs:\n  x:\n    steps:\n"
            "      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8\n"
            "        with:\n"
            f'          toolchain: "{PINNED}"\n'
            "          components: rustfmt, clippy\n"
        )
        self.assertEqual(pin.check_file(Path("ci.yml"), text, PINNED), [])

    def test_ok_action_reads_toml_only(self) -> None:
        text = (
            "jobs:\n  x:\n    steps:\n"
            "      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8\n"
            "        with:\n"
            "          components: rustfmt, clippy\n"
        )
        self.assertEqual(pin.check_file(Path("ci.yml"), text, PINNED), [])

    def test_fail_floating_action_ref(self) -> None:
        text = (
            "jobs:\n  x:\n    steps:\n"
            "      - uses: dtolnay/rust-toolchain@stable\n"
            "        with:\n"
            "          components: rustfmt, clippy\n"
        )
        errs = pin.check_file(Path("ci.yml"), text, PINNED)
        self.assertEqual(len(errs), 1)
        self.assertIn("@stable", errs[0])

    def test_fail_floating_toolchain_with(self) -> None:
        text = (
            "jobs:\n  x:\n    steps:\n"
            "      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8\n"
            "        with:\n"
            "          toolchain: stable\n"
        )
        errs = pin.check_file(Path("ci.yml"), text, PINNED)
        self.assertTrue(any("toolchain: stable" in e for e in errs))

    def test_fail_mismatched_version(self) -> None:
        text = (
            "jobs:\n  x:\n    steps:\n"
            "      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8\n"
            "        with:\n"
            '          toolchain: "1.85.0"\n'
        )
        errs = pin.check_file(Path("ci.yml"), text, PINNED)
        self.assertTrue(any("1.85.0" in e and PINNED in e for e in errs))

    def test_fail_floating_shell(self) -> None:
        text = "run: |\n  rustup default stable\n  cargo +nightly build\n"
        errs = pin.check_file(Path("ci.yml"), text, PINNED)
        self.assertGreaterEqual(len(errs), 2)


class CheckTree(unittest.TestCase):
    def test_clean_tree(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                f'[toolchain]\nchannel = "{PINNED}"\n',
                {
                    "ci.yml": (
                        "jobs:\n  x:\n    steps:\n"
                        "      - uses: dtolnay/rust-toolchain@abc123\n"
                        "        with:\n"
                        f'          toolchain: "{PINNED}"\n'
                    )
                },
            )
            self.assertEqual(pin.check_tree(root), [])

    def test_missing_toml(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".github" / "workflows").mkdir(parents=True)
            (root / ".github" / "workflows" / "ci.yml").write_text("jobs: {}\n", encoding="utf-8")
            errs = pin.check_tree(root)
            self.assertTrue(any("missing rust-toolchain.toml" in e for e in errs))

    def test_canary_floating_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                f'[toolchain]\nchannel = "{PINNED}"\n',
                {"ci.yml": "- uses: dtolnay/rust-toolchain@stable\n"},
            )
            errs = pin.check_tree(root)
            self.assertTrue(any("@stable" in e for e in errs))

    def test_canary_mismatch_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                f'[toolchain]\nchannel = "{PINNED}"\n',
                {
                    "ci.yml": (
                        "- uses: dtolnay/rust-toolchain@abc\n"
                        "  with:\n"
                        '    toolchain: "1.70.0"\n'
                    )
                },
            )
            errs = pin.check_tree(root)
            self.assertTrue(any("1.70.0" in e for e in errs))


class Main(unittest.TestCase):
    def test_main_ok(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                f'[toolchain]\nchannel = "{PINNED}"\n',
                {
                    "ok.yml": (
                        "- uses: dtolnay/rust-toolchain@deadbeef\n"
                        "  with:\n"
                        f'    toolchain: "{PINNED}"\n'
                    )
                },
            )
            self.assertEqual(pin.main(["--root", str(root)]), 0)

    def test_main_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_tree(
                root,
                f'[toolchain]\nchannel = "{PINNED}"\n',
                {"bad.yml": "- uses: dtolnay/rust-toolchain@stable\n"},
            )
            self.assertEqual(pin.main(["--root", str(root)]), 1)


if __name__ == "__main__":
    unittest.main()
