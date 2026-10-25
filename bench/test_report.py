"""The report's rules: a fixed matrix, breaches fail, a paired bootstrap over tasks."""

import unittest

import report

TASKS = report.expected_tasks()


def row(task, mode, passed=True, exit=0, valid=True, **more):
    return dict(task=task, mode=mode, passed=passed, exit=exit, valid=valid, **more)


def full(over=None):
    over = over or {}
    return [row(t, m, **over.get((t, m), {})) for t in TASKS for m in ("solo", "hierarchy")]


class Matrix(unittest.TestCase):
    def test_a_missing_extra_or_twice_recorded_run_is_an_error(self):
        rows = full()[1:] + [row("99-nope", "solo"), full()[2]]
        out, code = report.report(rows, ["solo", "hierarchy"], partial=False)
        self.assertEqual(code, 1)
        self.assertTrue(any("extra run: 99-nope" in e for e in out["errors"]))
        self.assertTrue(any("recorded twice" in e for e in out["errors"]))
        self.assertEqual(len(out["missing"]), 1)

    def test_partial_reports_the_gaps_but_never_hides_an_error(self):
        out, code = report.report(full()[2:], ["solo", "hierarchy"], partial=True)
        self.assertEqual((code, len(out["missing"])), (0, 2))
        _, code = report.report(full() + [full()[0]], ["solo", "hierarchy"], partial=True)
        self.assertEqual(code, 1)


class Success(unittest.TestCase):
    def test_a_correct_answer_that_broke_the_rules_is_a_failed_task(self):
        for broke in ({"exit": 1}, {"exit": None}, {"valid": False}):
            self.assertFalse(report.succeeded(row("t", "hierarchy", **broke)), broke)
        self.assertTrue(report.succeeded(row("t", "solo")))
        self.assertFalse(report.succeeded(row("t", "solo", passed=False)))


class Estimand(unittest.TestCase):
    def test_the_bootstrap_resamples_tasks_and_says_what_it_estimates(self):
        lose = {(t, "hierarchy"): {"passed": False} for t in TASKS[:6]}
        out, code = report.report(full(lose), ["solo", "hierarchy"], partial=False)
        e = out["estimand"]
        self.assertEqual(code, 0)
        self.assertEqual(e["groups"], len(TASKS))
        self.assertAlmostEqual(e["difference"], -6 / len(TASKS))
        lo, hi = e["interval_95"]
        self.assertTrue(lo <= e["difference"] <= hi < 0, e)
        self.assertIn("hierarchy minus solo", e["what"])
        # Seeded: the same rows give the same interval.
        self.assertEqual(report.report(full(lose), ["solo", "hierarchy"], False)[0]["estimand"], e)
        self.assertNotIn("score", out)


if __name__ == "__main__":
    unittest.main()
