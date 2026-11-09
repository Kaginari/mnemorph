"""Mnemorph as a Harbor installed agent: see README.md.

    harbor run -d terminal-bench@2.0 -m openrouter/<model> \
        --agent-import-path bench.harbor.mnemorph_agent:Mnemorph --ak mode=hierarchy

The binaries come from $MNEMORPH_DIST on the host, built for the oldest glibc a task image has;
the key from $OPENROUTER_API_KEY. `mode` is `solo` (the default) or `hierarchy`.
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
MODES = ("solo", "hierarchy")
# The hierarchy's ask, as the large bench words it: Rimuru writes the Elf a brief.
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
        "mk rimuru/rimuru '# rimuru\\n\\n- **Race:** rimuru\\n'; "
        "mk elf/e \"# elf-e\\n\\n- **Race:** elf\\n- **Reports to:** rimuru\\n- **Territory:** $g\\n\"; "
        "mk orc/o \"# orc-o\\n\\n- **Race:** orc\\n- **Reports to:** elf-e\\n- **Domain:** $g\\n\"; "
        "mk slime/workspace \"# slime-workspace\\n\\n- **Race:** slime\\n- **Reports to:** orc-o"
        "\\n- **Territory:** $g\\n\""
    )
    return card + " && mnemorphd roster adopt --world . --yes >/dev/null"


def minutes(logs_dir, tasks=None):
    """The session's clock for this trial: 85% of the task's own agent timeout, so Mnemorph's last
    salvage turn runs before Harbor kills it; two hours when the task is not found. Harbor names
    the trial's directory `<task>__<id>`, and `$TB_TASKS` holds the tasks' `task.toml` files."""
    import tomllib
    tasks = tasks or os.environ.get("TB_TASKS")
    task = Path(logs_dir).parent.name.split("__")[0]
    try:
        with open(Path(tasks) / task / "task.toml", "rb") as f:
            seconds = tomllib.load(f)["agent"]["timeout_sec"]
    except (TypeError, OSError, KeyError, ValueError):
        return 120
    return max(5, int(seconds * 0.85 / 60))


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
        setup = "mkdir -p %s/mnemorph && printf '%%s' %s > %s/mnemorph/config.toml" % (
            CONFIG_HOME, shlex.quote(config(model, minutes(self.logs_dir))), CONFIG_HOME)
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
        keep = ("mkdir -p /logs/agent/loop; cp -r .mnemorph/instruments/loop/. /logs/agent/loop/ "
                "2>/dev/null; cp .mnemorph/log.md /logs/agent/log.md 2>/dev/null; true")
        await self.exec_as_agent(
            environment,
            command="%s && { ( while sleep 30; do %s; done ) & copier=$!; "
            "mnemorph run %s --world . --json > %s 2> /logs/agent/mnemorph.stderr; "
            "code=$?; kill $copier 2>/dev/null; %s; exit $code; }"
            % (" && ".join(steps), keep, shlex.quote(ask), EVENTS, keep),
            env={"XDG_CONFIG_HOME": CONFIG_HOME,
                 "XDG_STATE_HOME": CONFIG_HOME + "/state",
                 "OPENROUTER_API_KEY": os.environ.get("OPENROUTER_API_KEY", "")},
        )

    def populate_context_post_run(self, context: AgentContext) -> None:
        events = Path(self.logs_dir) / "mnemorph.jsonl"
        if not events.exists():
            return
        for line in events.read_text(errors="replace").splitlines():
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if isinstance(event, dict) and event.get("type") == "result":
                usage = event.get("usage") or {}
                context.n_input_tokens = usage.get("input")
                context.n_output_tokens = usage.get("output")
                context.n_cache_tokens = usage.get("cacheRead")
                context.cost_usd = usage.get("usd")
