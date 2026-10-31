"""The report's rules: a fixed matrix, breaches fail, a paired bootstrap over tasks and
Newcombe's paired interval beside it."""

import json
import unittest

import report
import run

TASKS = report.expected_tasks()


def row(task, mode, passed=True, exit=0, valid=True, **more):
    return dict(task=task, mode=mode, passed=passed, exit=exit, valid=valid, **more)


def full(over=None):
    over = over or {}
    return [row(t, m, **over.get((t, m), {})) for t in TASKS for m in ("solo", "hierarchy")]


def arms(over=None, modes=("solo",)):
    """Every task in `modes`, law on and law off, each run saying its own law."""
    over = over or {}
    return [row(t, m, law=l, law_said=l == "on", **over.get((t, m, l), {}))
            for t in TASKS for m in modes for l in ("on", "off")]


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
        [e] = out["estimands"]
        self.assertEqual(code, 0)
        self.assertEqual(e["groups"], len(TASKS))
        self.assertAlmostEqual(e["difference"], -6 / len(TASKS))
        lo, hi = e["interval_95"]
        self.assertTrue(lo <= e["difference"] <= hi < 0, e)
        self.assertIn("hierarchy minus solo", e["what"])
        self.assertEqual(e["discordant"], [0, 6])
        # Seeded: the same rows give the same interval.
        self.assertEqual(report.report(full(lose), ["solo", "hierarchy"], False)[0]["estimands"], [e])
        self.assertNotIn("score", out)


class Causes(unittest.TestCase):
    def test_each_ending_is_told_by_its_cause(self):
        said = lambda e: row("t", "solo", exit=1, stderr="  ✗ x\nmnemorph: %s\n" % e)
        for stderr_ending, want in [
            ("escalate: more than two failed acts in a row", "escalated"),
            ("checkpoint: the budget is spent", "checkpoint"),
            ("failed: the end gate failed twice: traits hold", "gate"),
            ("failed: the end gate could not verify: no kernel guard", "machine"),
            ("failed: the end gate could not verify twice: traits unverified", "gate"),
            ("failed: the model refused to continue", "refused"),
            ("failed: the conversation is malformed: two user turns", "harness bug"),
            ("failed: the provider answered 502", "provider"),
            ("interrupted", "interrupted"),
        ]:
            self.assertEqual(report.cause(said(stderr_ending)), want, stderr_ending)
        self.assertEqual(report.cause(row("t", "solo", exit=None)), "hang")
        self.assertEqual(report.cause(row("t", "hierarchy", valid=False)), "never delegated")
        self.assertEqual(report.cause(row("t", "solo", exit=1, ending="escalate: x")), "escalated")

    def test_a_run_the_provider_stopped_is_rerun_never_counted(self):
        infra = row(TASKS[0], "solo", exit=1, stderr="mnemorph: failed: the provider answered 502\n")
        rows = [infra] + full()[1:]
        out, code = report.report(rows, ["solo", "hierarchy"], partial=False)
        self.assertEqual((code, out["rerun"]), (1, [(TASKS[0], "solo", "on")]))
        out, code = report.report(rows, ["solo", "hierarchy"], partial=True)
        self.assertEqual((code, out["arms"]["solo"]["runs"]), (0, len(TASKS) - 1))
        # A later row stands in for it; a second judged row is still an error.
        out, code = report.report(rows + [full()[0]], ["solo", "hierarchy"], partial=False)
        self.assertEqual((code, out["rerun"], out["errors"]), (0, [], []))
        _, code = report.report(full() + [full()[0]], ["solo", "hierarchy"], partial=False)
        self.assertEqual(code, 1)


class LawArm(unittest.TestCase):
    def test_a_missing_arm_is_a_matrix_error(self):
        law_on_only = [r for r in arms() if r["law"] == "on"]
        out, code = report.report(law_on_only, ["solo"], partial=False, laws=["on", "off"])
        self.assertEqual(code, 1)
        self.assertEqual(len(out["missing"]), len(TASKS))
        self.assertTrue(all(k[2] == "off" for k in out["missing"]))

    def test_a_run_that_said_another_law_than_its_arm_is_an_error(self):
        rows = arms()
        rows[1] = dict(rows[1], law_said=True)  # labelled off, but the run held the law
        out, code = report.report(rows, ["solo"], partial=True, laws=["on", "off"])
        self.assertEqual(code, 1)
        self.assertTrue(any("mislabelled arm" in e for e in out["errors"]), out["errors"])

    def test_both_arms_pair_by_task_and_a_row_before_the_arm_ran_with_the_law(self):
        lose = {(t, "solo", "off"): {"passed": False} for t in TASKS[:5]}
        out, code = report.report(arms(lose), ["solo"], partial=False, laws=["on", "off"])
        self.assertEqual(code, 0)
        [e] = out["estimands"]
        self.assertIn("law on minus law off, solo", e["what"])
        self.assertAlmostEqual(e["difference"], 5 / len(TASKS))
        self.assertEqual(e["discordant"], [5, 0])
        self.assertEqual(set(out["arms"]), {"solo, law on", "solo, law off"})
        self.assertEqual(report.law(row("t", "solo")), "on")


class Breach(unittest.TestCase):
    def test_an_answered_run_that_broke_a_rule_is_a_failed_task(self):
        self.assertEqual(report.RULES, ("envelope", "gate", "sandbox"))
        for rule in report.RULES:
            r = row("t", "solo", refusals={rule: 1})
            self.assertEqual(report.cause(r), "breach", rule)
            self.assertFalse(report.succeeded(r), rule)
        self.assertTrue(report.succeeded(row("t", "solo", refusals={})))
        self.assertTrue(report.judged(row("t", "solo", refusals={"gate": 2})))

    def test_the_report_shows_breaches_per_arm(self):
        broke = {(TASKS[0], "solo", "off"): {"refusals": {"gate": 1}},
                 (TASKS[1], "solo", "off"): {"refusals": {"sandbox": 2}, "exit": 1,
                                             "ending": "escalate: x"}}
        out, code = report.report(arms(broke), ["solo"], partial=False, laws=["on", "off"])
        self.assertEqual(code, 0)
        off, on = out["arms"]["solo, law off"], out["arms"]["solo, law on"]
        self.assertEqual((off["breaches"], on["breaches"]), (2, 0))
        self.assertEqual(off["refusals"], {"gate": 1, "sandbox": 2})
        self.assertEqual(off["failed_by"], {"breach": 1, "escalated": 1})
        self.assertAlmostEqual(off["success"], 1 - 2 / len(TASKS))

    def test_run_reads_the_ending_the_law_and_each_refusal_from_json(self):
        lines = [
            {"type": "toolResult", "body": "rimuru", "id": "b1", "refused": "gate"},
            {"type": "toolResult", "body": "slime-s", "id": "b2", "refused": "sandbox"},
            {"type": "toolResult", "body": "rimuru", "id": "w1"},
            {"type": "turnEnded", "body": "elf-e", "ending": "answered"},
            {"type": "turnEnded", "body": "rimuru", "ending": "answered"},
            {"type": "result", "ending": "answered", "law": False, "refusals": {"gate": 1}},
        ]
        said, endings, refusals = run.read_json(
            "\n".join(json.dumps(l) for l in lines) + "\n{torn")
        self.assertEqual((said["law"], endings, refusals),
                         (False, ["answered"], {"gate": 1, "sandbox": 1}))


    def test_a_solo_escalation_of_an_outward_act_is_no_breach(self):
        # Solo is refused `git push` by its shelf and hands it to the Kijin: the designed path.
        escalated = row("t", "solo", refusals={"shelf": 1}, escalations=1)
        self.assertEqual(report.cause(escalated), "answered")
        self.assertTrue(report.succeeded(escalated))
        self.assertEqual(report.cause(row("t", "solo", refusals={"shelf": 1, "gate": 1})), "breach")
        out, _ = report.report([dict(r, refusals={"shelf": 1}) if r["task"] == TASKS[0] else r
                                for r in full()], ["solo", "hierarchy"], partial=False)
        solo = out["arms"]["solo"]
        self.assertEqual((solo["breaches"], solo["refusals"]), (0, {"shelf": 1}))


class MixedArms(unittest.TestCase):
    def test_a_run_whose_law_beats_disagree_is_refused(self):
        rows = arms()
        rows[1] = dict(rows[1], law_beats=[{"on": False, "resumed": False},
                                           {"on": True, "resumed": True}])
        out, code = report.report(rows, ["solo"], partial=True, laws=["on", "off"])
        self.assertEqual(code, 1)
        self.assertTrue(any("mixed arms" in e for e in out["errors"]), out["errors"])
        rows[1] = dict(rows[1], law_beats=[{"on": False, "resumed": False}] * 4)
        self.assertEqual(report.report(rows, ["solo"], partial=False, laws=["on", "off"])[1], 0)

    def test_run_reads_every_law_beat_of_the_world_s_journals(self):
        import pathlib
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            loop = pathlib.Path(d) / ".mnemorph" / "instruments" / "loop"
            loop.mkdir(parents=True)
            (loop / "r1.jsonl").write_text('{"beat":"law","on":false}\n{"beat":"ask","text":"x"}\n'
                                           '{"beat":"law","on":true,"resumed":true}\n')
            (loop / "elf-e-1.jsonl").write_text('{"beat":"law","on":false}\ntorn')
            self.assertEqual(run.law_beats(pathlib.Path(d)), [
                {"on": False, "resumed": False},
                {"on": False, "resumed": False}, {"on": True, "resumed": True}])


class Newcombe(unittest.TestCase):
    def test_no_pair_is_no_interval(self):
        self.assertIsNone(report.newcombe(0, 0, 0, 0))

    def test_it_matches_the_published_values(self):
        # Newcombe 1998, Statistics in Medicine 17:2635, the table on p. 2641: 20 pairs both
        # successes, 12 the first only, 2 the second only, 16 neither — method 10 gives
        # 0.0562 to 0.3292.
        lo, hi = report.newcombe(20, 12, 2, 16)
        self.assertEqual((round(lo, 4), round(hi, 4)), (0.0562, 0.3292))
        # statsmodels' confint_proportions_paired, checked against R's ratesci: 37 patients, 10
        # respond to both, 5 to A alone, 2 to B alone.
        lo, hi = report.newcombe(10, 5, 2, 20)
        self.assertAlmostEqual(lo, -0.0643926050711, places=10)
        self.assertAlmostEqual(hi, 0.2211160178958, places=10)

    def test_it_stays_in_range_with_no_discordant_pair(self):
        lo, hi = report.newcombe(30, 0, 0, 0)
        self.assertTrue(-1 <= lo < 0 < hi <= 1, (lo, hi))

    def test_the_report_states_the_estimand_the_groups_and_both_intervals(self):
        lose = {(t, "hierarchy"): {"passed": False} for t in TASKS[:3]}
        out, _ = report.report(full(lose), ["solo", "hierarchy"], partial=False)
        [e] = out["estimands"]
        lo, hi = e["newcombe_95"]
        self.assertTrue(lo < e["difference"] < hi, e)
        self.assertIn("Newcombe hybrid score", e["newcombe_method"])
        self.assertIn("bootstrap", e["method"])
        import contextlib
        import io
        shown = io.StringIO()
        with contextlib.redirect_stdout(shown):
            report.show(out, partial=False)
        text = shown.getvalue()
        for said in ("estimand: task success rate, hierarchy minus solo",
                     "%d task groups" % len(TASKS), "paired percentile bootstrap",
                     "Newcombe hybrid score, paired (1998, method 10)", "breaches 0"):
            self.assertIn(said, text)


if __name__ == "__main__":
    unittest.main()
