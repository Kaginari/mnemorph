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
    def test_a_correct_file_from_a_run_that_did_not_end_answered_is_a_failed_task(self):
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
        self.assertEqual(e["discordant"], [0, 6])
        # Seeded: the same rows give the same interval.
        self.assertEqual(report.report(full(lose), ["solo", "hierarchy"], False)[0]["estimand"], e)
        self.assertNotIn("score", out)


class Causes(unittest.TestCase):
    def test_each_ending_is_told_by_its_cause(self):
        said = lambda e: row("t", "solo", exit=1, stderr="  ✗ x\nmnemorph: %s\n" % e)
        for stderr_ending, want in [
            ("escalate: more than two failed acts in a row", "escalated"),
            ("checkpoint: the budget is spent", "checkpoint"),
            ("failed: the end gate failed twice: traits hold", "gate"),
            ("failed: the end gate could not verify: no kernel guard", "machine"),
            ("failed: the provider answered 502", "provider"),
            ("interrupted", "interrupted"),
        ]:
            self.assertEqual(report.cause(said(stderr_ending)), want, stderr_ending)
        self.assertEqual(report.cause(row("t", "solo", exit=None)), "timeout")
        self.assertEqual(report.cause(row("t", "hierarchy", valid=False)), "never delegated")
        self.assertEqual(report.cause(row("t", "solo", exit=1, ending="escalate: x")), "escalated")

    def test_a_run_the_provider_stopped_is_rerun_never_counted(self):
        infra = row(TASKS[0], "solo", exit=1, stderr="mnemorph: failed: the provider answered 502\n")
        rows = [infra] + full()[1:]
        out, code = report.report(rows, ["solo", "hierarchy"], partial=False)
        self.assertEqual((code, out["rerun"]), (1, [(TASKS[0], "solo")]))
        out, code = report.report(rows, ["solo", "hierarchy"], partial=True)
        self.assertEqual((code, out["arms"]["solo"]["runs"]), (0, len(TASKS) - 1))
        # A later row stands in for it; a second judged row is still an error.
        out, code = report.report(rows + [full()[0]], ["solo", "hierarchy"], partial=False)
        self.assertEqual((code, out["rerun"], out["errors"]), (0, [], []))
        _, code = report.report(full() + [full()[0]], ["solo", "hierarchy"], partial=False)
        self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main()
