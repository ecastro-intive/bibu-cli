"""Tests for post_review.py. Run: python3 -m unittest discover -s .github/scripts"""

import unittest

import post_review as pr

# New-side lines in this patch: 10 (context), 11 and 12 (added), 13 (context), then 40 and 41.
# The removed line does not exist on the new side and must not be commentable.
PATCH = """@@ -10,3 +10,4 @@ fn a() {
 context ten
-removed
+added eleven
+added twelve
 context thirteen
@@ -40,2 +40,2 @@ fn b() {
 context forty
+added forty one
"""


def finding(file="src/a.rs", line=11, severity="blocker", message="bad"):
    return {"file": file, "line": line, "severity": severity, "message": message}


def output(findings, summary="Adds a thing."):
    return {"summary": summary, "findings": findings}


class ValidLines(unittest.TestCase):
    def test_collects_context_and_added_lines_of_every_hunk(self):
        self.assertEqual(pr.valid_lines(PATCH), {10, 11, 12, 13, 40, 41})

    def test_removed_lines_are_not_commentable(self):
        self.assertNotIn(14, pr.valid_lines(PATCH))

    def test_missing_patch_means_no_commentable_lines(self):
        self.assertEqual(pr.valid_lines(None), set())

    def test_single_line_hunk_header_without_count(self):
        self.assertEqual(pr.valid_lines("@@ -1 +1 @@\n+only\n"), {1})

    def test_no_newline_marker_is_ignored(self):
        patch = "@@ -1 +1 @@\n+only\n\\ No newline at end of file\n"
        self.assertEqual(pr.valid_lines(patch), {1})


class BuildReview(unittest.TestCase):
    files = {"src/a.rs": PATCH}

    def test_finding_on_a_diff_line_becomes_an_inline_comment(self):
        review = pr.build_review(output([finding()]), self.files, "abc123")
        self.assertEqual(review["commit_id"], "abc123")
        self.assertEqual(review["event"], "COMMENT")
        self.assertEqual(len(review["comments"]), 1)
        comment = review["comments"][0]
        self.assertEqual(
            (comment["path"], comment["line"], comment["side"]),
            ("src/a.rs", 11, "RIGHT"),
        )

    def test_severity_is_visible_in_the_comment(self):
        blocker = pr.build_review(output([finding(severity="blocker")]), self.files, "x")
        should = pr.build_review(output([finding(severity="should_fix")]), self.files, "x")
        self.assertIn("blocker", blocker["comments"][0]["body"].lower())
        self.assertIn("should fix", should["comments"][0]["body"].lower())
        self.assertIn("bad", blocker["comments"][0]["body"])

    def test_finding_outside_the_diff_goes_to_the_body_not_inline(self):
        outside = finding(line=14, message="outside message")
        review = pr.build_review(output([outside]), self.files, "x")
        self.assertEqual(review["comments"], [])
        self.assertIn("src/a.rs:14", review["body"])
        self.assertIn("outside message", review["body"])

    def test_finding_in_a_file_not_in_the_pr_goes_to_the_body(self):
        review = pr.build_review(output([finding(file="src/other.rs")]), self.files, "x")
        self.assertEqual(review["comments"], [])
        self.assertIn("src/other.rs:11", review["body"])

    def test_file_without_a_patch_goes_to_the_body(self):
        review = pr.build_review(output([finding()]), {"src/a.rs": None}, "x")
        self.assertEqual(review["comments"], [])
        self.assertIn("src/a.rs:11", review["body"])

    def test_body_has_summary_and_blocker_count(self):
        findings = [finding(), finding(line=12), finding(line=13, severity="should_fix")]
        review = pr.build_review(output(findings), self.files, "x")
        self.assertIn("Adds a thing.", review["body"])
        self.assertIn("2 blockers", review["body"])

    def test_no_findings_says_so_and_has_no_comments(self):
        review = pr.build_review(output([]), self.files, "x")
        self.assertEqual(review["comments"], [])
        self.assertIn("No issues found", review["body"])
        self.assertIn("No blockers", review["body"])

    def test_one_blocker_is_singular(self):
        review = pr.build_review(output([finding()]), self.files, "x")
        self.assertIn("1 blocker", review["body"])
        self.assertNotIn("1 blockers", review["body"])


class Extract(unittest.TestCase):
    def test_returns_the_structured_output(self):
        data = {"is_error": False, "structured_output": output([])}
        self.assertEqual(pr.extract(data), output([]))

    def test_error_response_raises(self):
        with self.assertRaises(pr.ReviewError):
            pr.extract({"is_error": True, "result": "boom"})

    def test_missing_structured_output_raises(self):
        with self.assertRaises(pr.ReviewError):
            pr.extract({"is_error": False, "result": "free text"})


if __name__ == "__main__":
    unittest.main()
