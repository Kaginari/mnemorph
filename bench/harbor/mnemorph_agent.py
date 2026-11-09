"""Mnemorph as a Harbor installed agent: see README.md.

    harbor run -d terminal-bench@2.0 -m openrouter/<model> \
        --agent-import-path bench.harbor.mnemorph_agent:Mnemorph --ak mode=hierarchy

The binaries come from $MNEMORPH_DIST on the host, built for the oldest glibc a task image has;
the key from $OPENROUTER_API_KEY. `mode` is `solo` (the default), `hierarchy`, or `reviewed`: a
solo pass, then a second solo session that reviews it against the task.
"""

import json
import os
import shlex
from pathlib import Path

from harbor.agents.installed.base import BaseInstalledAgent
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext

BINARIES = ("mnemorph", "mnemorphd", "mnemorph-guard")
CONFIG_HOME = "/tmp/mnemorph-config"
EVENTS = "/logs/agent/mnemorph.jsonl"
MODES = ("solo", "hierarchy", "reviewed")
# The reviewed arm's second session: a fresh solo run that checks the first pass against the task.
# The chain's wins on the bench came from a second look; this is that look without the chain.
REVIEW = (
    "A first pass at this task has already run in this repository. Review its work as a strict "
    "reviewer: re-read every requirement of the task below, run the checks it implies, its edge "
    "cases included, and fix whatever fails or is missing. Change nothing that already meets the "
    "task. The task:\n\n"
)
# The hierarchy's ask, as the large bench words it: Summoner writes the Elf a brief.
DELEGATE = (
    "Hand the work of this ask to elf-e with the commission tool, as a brief you write for it, "
    "then answer with its report:\n\n"
)


def team():
    """Shell that writes a hierarchy over the task's folder, as it is when the run starts: an Elf,
    an Orc and `slime-workspace`, each over every top-level directory; the workspace Slime holds
    the root files too. A task's folder is no codebase with parts, so one of each."""
    card = (
        "mk() { mkdir -p .mnemorph/$1 && printf '%b' \"$2\" > .mnemorph/$1/README.md; }; "
        "g=$(find . -mindepth 1 -maxdepth 1 -type d ! -name .git ! -name .mnemorph -printf '%f/ ' "
        "| sort | tr '\\n' ' '); "
        "mk summoner/summoner '# summoner\\n\\n- **Race:** summoner\\n'; "
        "mk elf/e \"# elf-e\\n\\n- **Race:** elf\\n- **Reports to:** summoner\\n- **Territory:** $g\\n\"; "
        "mk orc/o \"# orc-o\\n\\n- **Race:** orc\\n- **Reports to:** elf-e\\n- **Domain:** $g\\n\"; "
        "mk slime/workspace \"# slime-workspace\\n\\n- **Race:** slime\\n- **Reports to:** orc-o"
        "\\n- **Territory:** $g\\n\""
    )
    return card + " && mnemorphd roster adopt --world . --yes >/dev/null"


def minutes(logs_dir, tasks=None):
    """The session's clock for this trial: the task's own agent timeout less 90 seconds, so
    Mnemorph's last salvage turn runs before Harbor kills it, and never under 5 minutes; two hours
    when the task is not found. (85% of it left a chain too little on a 15-minute task.) Harbor names
    the trial's directory `<task>__<id>`, and `$TB_TASKS` holds the tasks' `task.toml` files."""
    import tomllib
    tasks = tasks or os.environ.get("TB_TASKS")
    task = Path(logs_dir).parent.name.split("__")[0]
    try:
        with open(Path(tasks) / task / "task.toml", "rb") as f:
            seconds = tomllib.load(f)["agent"]["timeout_sec"]
    except (TypeError, OSError, KeyError, ValueError):
        return 120
    return max(5, int((seconds - 90) / 60))


def config(model, session_minutes=120):
    """The human's global file for one task: the provider, the model, and the container profile."""
    return (
        "[providers.openrouter]\n"
        'kind = "openai"\n'
        'base_url = "https://openrouter.ai/api"\n'
        'key_env = "OPENROUTER_API_KEY"\n'
        "[models]\n"
        'provider = "openrouter"\n'
        'default = "%s"\n'
        "[sandbox]\n"
        "disposable = true\n"
        # The large bench's steps: a chain given fewer than solo measures the starving.
        "[steps]\n"
        "solo = 200\n"
        "elf = 80\n"
        "orc = 80\n"
        "slime = 80\n"
        "[limits]\n"
        "session_minutes = %d\n" % (model, session_minutes)
    )


class Mnemorph(BaseInstalledAgent):
    def __init__(self, *args, mode="solo", **kwargs):
        if mode not in MODES:
            raise ValueError("mode is one of %s, not %r" % (", ".join(MODES), mode))
        self.mode = mode
        super().__init__(*args, **kwargs)

    @staticmethod
    def name() -> str:
        return "mnemorph"

    def get_version_command(self) -> str | None:
        return "mnemorph --version"

    async def install(self, environment: BaseEnvironment) -> None:
        await self.ensure_system_dependencies(environment, ("git",))
        dist = Path(os.environ["MNEMORPH_DIST"])
        for name in BINARIES:
            await environment.upload_file(dist / name, "/usr/local/bin/" + name)
        await self.exec_as_root(
            environment,
            command="chmod 755 " + " ".join("/usr/local/bin/" + b for b in BINARIES),
        )

    async def run(self, instruction: str, environment: BaseEnvironment, context: AgentContext) -> None:
        model = (self.model_name or "").removeprefix("openrouter/")
        if not model:
            raise ValueError("a model is needed: -m openrouter/<model>")
        total = minutes(self.logs_dir)
        # The reviewed arm splits the clock: the first pass a little over half, the review the rest.
        first = max(3, int(total * 0.55)) if self.mode == "reviewed" else total
        # Filled twice, the config home now and the file later: printf's own `%s` survives both.
        write = "mkdir -p %s/mnemorph && printf '%%%%s' %%s > %s/mnemorph/config.toml" % (
            CONFIG_HOME, CONFIG_HOME)
        setup = write % shlex.quote(config(model, first))
        snapshot = (
            "if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then "
            "git init -q && git add -A && git -c user.name=bench -c user.email=bench@localhost "
            "commit -q --allow-empty -m 'the task before the run'; fi"
        )
        ask = instruction
        steps = [setup, snapshot]
        if self.mode == "hierarchy":
            steps.append(team())
            ask = DELEGATE + instruction
        # The run's journals outlive the container: copied beside the events, whatever the ending.
        # Every 30 seconds too: a run Harbor kills at the task's timeout never reaches the end.
        # The usage ledger too: a run killed before its result line still leaves what it spent.
        keep = ("mkdir -p /logs/agent/loop; cp -r .mnemorph/instruments/loop/. /logs/agent/loop/ "
                "2>/dev/null; cp .mnemorph/instruments/usage.jsonl /logs/agent/usage.jsonl "
                "2>/dev/null; cp .mnemorph/log.md /logs/agent/log.md 2>/dev/null; true")
        runs = "mnemorph run %s --world . --json > %s 2> /logs/agent/mnemorph.stderr; code=$?; " % (
            shlex.quote(ask), EVENTS)
        if self.mode == "reviewed":
            runs += "%s; mnemorph run %s --world . --json >> %s 2>> /logs/agent/mnemorph.stderr; " \
                "code=$?; " % (write % shlex.quote(config(model, max(2, total - first - 1))),
                               shlex.quote(REVIEW + instruction), EVENTS)
        await self.exec_as_agent(
            environment,
            command="%s && { ( while sleep 30; do %s; done ) & copier=$!; "
            "%skill $copier 2>/dev/null; %s; exit $code; }"
            % (" && ".join(steps), keep, runs, keep),
            env={"XDG_CONFIG_HOME": CONFIG_HOME,
                 "XDG_STATE_HOME": CONFIG_HOME + "/state",
                 "OPENROUTER_API_KEY": os.environ.get("OPENROUTER_API_KEY", "")},
        )

    def populate_context_post_run(self, context: AgentContext) -> None:
        events = Path(self.logs_dir) / "mnemorph.jsonl"
        if not events.exists():
            return
        # A reviewed trial has two sessions, so two result lines: their usage is summed.
        totals = {"input": 0, "output": 0, "cacheRead": 0}
        usd, seen = 0.0, False
        for line in events.read_text(errors="replace").splitlines():
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if isinstance(event, dict) and event.get("type") == "result":
                usage = event.get("usage") or {}
                seen = True
                for key in totals:
                    totals[key] += usage.get(key) or 0
                usd += usage.get("usd") or 0.0
        if seen:
            context.n_input_tokens = totals["input"]
            context.n_output_tokens = totals["output"]
            context.n_cache_tokens = totals["cacheRead"]
            context.cost_usd = usd or None
