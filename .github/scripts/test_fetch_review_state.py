"""Tests for fetch_review_state.sh. Run: python3 -m unittest discover -s .github/scripts

The script is run for real, with a fake `gh` first on PATH. The fake answers the three calls the
script makes (find the newest artifact, say which run made it, download it) from environment
variables, so each test describes one situation on GitHub.
"""

import os
import subprocess
import tempfile
import unittest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fetch_review_state.sh")

FAKE_GH = r"""#!/bin/bash
case "$*" in
  *"/actions/artifacts?"*) printf '%s' "$FAKE_RUN_ID" ;;
  *"/actions/runs/"*)      printf '%s' "$FAKE_ORIGIN" ;;
  "run download"*)
    [ "$FAKE_DOWNLOAD" = fail ] && exit 1
    dir=prev; [ "$FAKE_LAYOUT" = nested ] && dir="prev/review-state-pr-7"
    mkdir -p "$dir" && printf '%s' '{"findings": []}' > "$dir/review-state.json" ;;
esac
"""

TRUSTED = ".github/workflows/claude-review.yml\tworkflow_run"


class FetchReviewState(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        bin_dir = os.path.join(self.tmp.name, "bin")
        os.mkdir(bin_dir)
        gh = os.path.join(bin_dir, "gh")
        with open(gh, "w", encoding="utf-8") as fh:
            fh.write(FAKE_GH)
        os.chmod(gh, 0o755)
        self.work = os.path.join(self.tmp.name, "work")
        os.mkdir(self.work)
        self.bin = bin_dir

    def run_script(self, **fake):
        env = {
            "PATH": f"{self.bin}:{os.environ['PATH']}",
            "REPO": "o/r",
            "PR": "7",
            "GH_TOKEN": "x",
            "FAKE_RUN_ID": "",
            "FAKE_ORIGIN": "",
            "FAKE_DOWNLOAD": "",
            "FAKE_LAYOUT": "",
        }
        env.update({k.upper(): v for k, v in fake.items()})
        done = subprocess.run(
            ["bash", "-e", SCRIPT], cwd=self.work, env=env, capture_output=True, text=True
        )
        return done

    def state_file(self):
        return os.path.join(self.work, "prev", "review-state.json")

    def test_downloads_the_state_written_by_this_workflow(self):
        done = self.run_script(fake_run_id="123", fake_origin=TRUSTED)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertTrue(os.path.isfile(self.state_file()))

    def test_finds_the_file_when_gh_nests_it_in_a_folder(self):
        done = self.run_script(fake_run_id="123", fake_origin=TRUSTED, fake_layout="nested")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertTrue(os.path.isfile(self.state_file()))

    def test_no_artifact_means_a_review_from_scratch(self):
        done = self.run_script(fake_run_id="")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertFalse(os.path.exists(self.state_file()))
        self.assertIn("No previous review state", done.stdout)

    def test_an_artifact_from_a_pull_request_workflow_is_ignored(self):
        origin = ".github/workflows/claude-review.yml\tpull_request"
        done = self.run_script(fake_run_id="123", fake_origin=origin)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertFalse(os.path.exists(self.state_file()))
        self.assertIn("::warning::", done.stdout)

    def test_an_artifact_from_another_workflow_is_ignored(self):
        done = self.run_script(fake_run_id="123", fake_origin=".github/workflows/ci.yml\tworkflow_run")
        self.assertFalse(os.path.exists(self.state_file()))
        self.assertIn("::warning::", done.stdout)

    def test_an_unknown_origin_is_ignored(self):
        done = self.run_script(fake_run_id="123", fake_origin="")
        self.assertFalse(os.path.exists(self.state_file()))
        self.assertIn("::warning::", done.stdout)

    def test_a_failed_download_warns_and_does_not_fail_the_job(self):
        done = self.run_script(fake_run_id="123", fake_origin=TRUSTED, fake_download="fail")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertFalse(os.path.exists(self.state_file()))
        self.assertIn("reviewing from scratch", done.stdout)


if __name__ == "__main__":
    unittest.main()
