#!/usr/bin/env bash
# Dotfiles are written with their $HOME and $PATH unexpanded, as a human writes them.
# shellcheck disable=SC2016
#
# The release-candidate smoke test: the BUILT binaries, run as a fresh machine's user would run
# them, in a throwaway HOME seeded with the dotfiles a real one carries — a .gitconfig with
# includes, aliases, a global hooks path and core.fsmonitor; ~/.config and ~/.local/state already
# holding other programs' files; a .bashrc that edits PATH — and a world that is a real git
# repository with history. The provider is a scripted one on loopback: no network, no key.
#
#   tools/rc-smoke.sh <dir holding mnemorph and mnemorphd | release archive .tar.gz> [version]
#
# With a version, both binaries must report it. The real HOME is never read or written: every
# path the binaries resolve (HOME, XDG_*) points into the temp dir, and the real HOME's Mnemorph
# directories are compared before and after. Exit 0 only when every check passed; each failure
# says what it saw.
set -euo pipefail

usage() { echo "usage: tools/rc-smoke.sh <bin-dir | archive.tar.gz> [version]" >&2; exit 2; }
if [ $# -lt 1 ] || [ $# -gt 2 ]; then usage; fi
input=$1
want_version=${2:-}
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
real_home=${HOME:?HOME is unset}

fails=0
pass() { printf '  ok    %s\n' "$1"; }
fail() { printf '  FAIL  %s\n' "$1" >&2; fails=$((fails + 1)); }
section() { printf '\n%s\n' "$1"; }

tmp=$(mktemp -d "${TMPDIR:-/tmp}/mnemorph-rc.XXXXXX")
tmp=$(cd "$tmp" && pwd -P)
case "$tmp/" in
  "$real_home"/*) echo "rc-smoke: the temp dir $tmp is inside the real HOME; set TMPDIR elsewhere" >&2; exit 2 ;;
esac
provider_pid=
cleanup() {
  if [ -n "$provider_pid" ]; then kill "$provider_pid" 2>/dev/null || true; fi
  if [ "$fails" -eq 0 ] && [ -z "${RC_SMOKE_KEEP:-}" ]; then
    chmod -R u+w "$tmp" 2>/dev/null || true
    rm -rf "$tmp"
  else
    echo "rc-smoke: kept $tmp for inspection" >&2
  fi
}
trap cleanup EXIT

# ---- the artifact -------------------------------------------------------------------------------
section "artifact: $input"
if [ -f "$input" ]; then
  mkdir "$tmp/unpacked"
  tar -xzf "$input" -C "$tmp/unpacked"
  bin=$(dirname "$(find "$tmp/unpacked" -type f -name mnemorph | head -n 1)")
  for f in LICENSE README.md; do
    if [ -f "$bin/$f" ]; then pass "archive carries $f"; else fail "archive carries $f"; fi
  done
elif [ -d "$input" ]; then
  bin=$(cd "$input" && pwd -P)
else
  usage
fi
for b in mnemorph mnemorphd; do
  if [ -x "$bin/$b" ]; then pass "$b is executable"; else fail "$b is missing from $bin"; exit 1; fi
done
if [ "$(uname -s)" = Linux ]; then
  # The sandbox finds its guard beside the running binary: without it, no command runs.
  if [ -x "$bin/mnemorph-guard" ]; then pass "mnemorph-guard ships beside mnemorph"; else fail "mnemorph-guard is missing from $bin"; fi
fi

# ---- the fresh HOME, with realistic dotfiles ----------------------------------------------------
home=$tmp/home
mkdir -p "$home"/.config/{nvim,htop,git,mnemorph} "$home"/.gitconfig.d "$home"/.githooks \
  "$home"/.local/state/{nvim,wireplumber} "$home"/.local/share/applications "$home"/.local/bin \
  "$home"/.cache "$home"/bin "$home"/code
cat > "$home/.gitconfig" <<'EOF'
[user]
	name = Sam Fresh
	email = sam@example.invalid
[include]
	path = ~/.gitconfig.d/aliases
[includeIf "gitdir:~/code/"]
	path = ~/.gitconfig.d/work
[init]
	defaultBranch = trunk
[core]
	hooksPath = ~/.githooks
	fsmonitor = true
	untrackedCache = true
	excludesFile = ~/.config/git/ignore
[pull]
	rebase = true
[commit]
	verbose = true
EOF
cat > "$home/.gitconfig.d/aliases" <<'EOF'
[alias]
	st = status -sb
	lg = log --oneline --graph --decorate
	co = checkout
	amend = commit --amend --no-edit
EOF
cat > "$home/.gitconfig.d/work" <<'EOF'
[user]
	email = sam@work.example.invalid
[color]
	ui = always
EOF
printf '*.swp\n.DS_Store\n.envrc\n' > "$home/.config/git/ignore"
# A global hook that leaves a mark each time it runs, so the harness running it is seen.
for hook in pre-commit commit-msg post-checkout; do
  printf '#!/bin/sh\necho "%s $PWD" >> "$HOME/.githooks.fired"\n' "$hook" > "$home/.githooks/$hook"
  chmod +x "$home/.githooks/$hook"
done
cat > "$home/.bashrc" <<'EOF'
# ~/.bashrc
case $- in *i*) ;; *) return ;; esac
export PATH="$HOME/bin:$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export EDITOR=nvim
alias ll='ls -alF'
alias gs='git st'
HISTCONTROL=ignoreboth
EOF
printf '[ -f ~/.bashrc ] && . ~/.bashrc\n' > "$home/.bash_profile"
printf 'export PATH="$HOME/bin:$PATH"\n' > "$home/.profile"
printf 'vim.opt.number = true\n' > "$home/.config/nvim/init.lua"
printf 'fields=0 48 17 18 38 39 40 2 46 47 49 1\n' > "$home/.config/htop/htoprc"
printf 'unrelated shada\n' > "$home/.local/state/nvim/shada"
printf '[wireplumber]\nrestore = true\n' > "$home/.local/state/wireplumber/restore-stream"
printf 'lesshst\n' > "$home/.local/state/lesshst"
printf '[Desktop Entry]\nName=Other\n' > "$home/.local/share/applications/other.desktop"

# The fresh environment: nothing of the caller's leaks in but PATH and the locale.
fresh_env=(env -i
  "HOME=$home" "USER=sam" "LOGNAME=sam" "SHELL=/bin/bash" "TERM=dumb" "LANG=${LANG:-C.UTF-8}"
  "PATH=$home/bin:$home/.local/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin"
  "TMPDIR=$tmp/tmpdir"
  "XDG_CONFIG_HOME=$home/.config" "XDG_STATE_HOME=$home/.local/state"
  "XDG_DATA_HOME=$home/.local/share" "XDG_CACHE_HOME=$home/.cache")
mkdir -p "$tmp/tmpdir"
in_home() { "${fresh_env[@]}" "$@"; }

# ---- a world that is a real repository with history ---------------------------------------------
world=$home/code/project
mkdir -p "$world/src"
printf '# project\n\nA small tool.\n' > "$world/README.md"
printf 'def main():\n    print("hello")\n' > "$world/src/app.py"
printf '__pycache__/\n*.pyc\n' > "$world/.gitignore"
(cd "$world" && in_home git init -q && in_home git add -A \
  && in_home git commit -qm "first commit" \
  && printf 'def add(a, b):\n    return a + b\n' > src/util.py && in_home git add src/util.py \
  && in_home git commit -qm "add util" 2>/dev/null) || { fail "seed the world's history"; exit 1; }
commits_before=$(cd "$world" && in_home git rev-list --count HEAD)
head_before=$(cd "$world" && in_home git rev-parse HEAD)
if [ "$commits_before" -eq 2 ]; then pass "the world is a git repository with $commits_before commits"
else fail "the world's history: $commits_before commits"; fi
: > "$home/.githooks.fired"

# ---- the scripted provider on loopback ----------------------------------------------------------
python3 "$here/rc-fake-provider.py" "$tmp/port" "$tmp/requests.jsonl" &
provider_pid=$!
for _ in $(seq 1 100); do [ -s "$tmp/port" ] && break; sleep 0.05; done
[ -s "$tmp/port" ] || { fail "the scripted provider did not start"; exit 1; }
port=$(cat "$tmp/port")
cat > "$home/.config/mnemorph/config.toml" <<EOF
[providers.local]
kind = "openai"
base_url = "http://127.0.0.1:$port"

[models]
provider = "local"
default = "rc-smoke"
EOF
pass "scripted provider on 127.0.0.1:$port"

# The real HOME's Mnemorph directories, as they stand: compared again at the end.
real_snapshot() {
  for d in "$real_home/.config/mnemorph" "$real_home/.local/state/mnemorph" \
    "$real_home/.local/share/mnemorph" "$real_home/.cache/mnemorph"; do
    if [ -e "$d" ]; then
      find "$d" -exec ls -ld {} + 2>/dev/null | sort
    else
      echo "absent $d"
    fi
  done
}
real_before=$(real_snapshot)
foreign_snapshot() {
  (cd "$home" && find .config/nvim .config/htop .config/git .local/state/nvim .local/state/wireplumber \
    .local/state/lesshst .local/share/applications .gitconfig .gitconfig.d .bashrc .profile -type f \
    -exec cksum {} + | sort)
}
foreign_before=$(foreign_snapshot)

# Run one command in the fresh HOME, with a ceiling: its exit code, stdout and stderr in files.
ceiling=${RC_SMOKE_TIMEOUT:-120}
run() {
  local name=$1; shift
  local rc=0
  if command -v timeout >/dev/null; then
    (cd "$world" && in_home timeout "$ceiling" "$@") > "$tmp/$name.out" 2> "$tmp/$name.err" || rc=$?
  else
    (cd "$world" && in_home "$@") > "$tmp/$name.out" 2> "$tmp/$name.err" || rc=$?
  fi
  echo "$rc" > "$tmp/$name.rc"
}
expect() { # name, wanted exit code, what
  local got; got=$(cat "$tmp/$1.rc")
  if [ "$got" = "$2" ]; then pass "$3 (exit $got)"; else
    fail "$3: exit $got, wanted $2"
    sed 's/^/        stdout: /' "$tmp/$1.out" | tail -n 20 >&2
    sed 's/^/        stderr: /' "$tmp/$1.err" | tail -n 20 >&2
  fi
}
says() { # name, stream (out|err), fixed string, what
  if grep -qF -- "$3" "$tmp/$1.$2"; then pass "$4"; else
    fail "$4: \`$3\` not in $2"
    sed "s/^/        $2: /" "$tmp/$1.$2" | tail -n 20 >&2
  fi
}

# ---- the commands --------------------------------------------------------------------------------
section "commands (HOME=$home)"
run version-tui "$bin/mnemorph" --version
expect version-tui 0 "mnemorph --version"
says version-tui out "mnemorph ${want_version}" "mnemorph --version says mnemorph ${want_version:-<version>}"

run version-core "$bin/mnemorphd" --version
expect version-core 0 "mnemorphd --version"
says version-core out "mnemorphd ${want_version}" "mnemorphd --version says mnemorphd ${want_version:-<version>}"
if grep -qE '\((unknown|[0-9a-f]{10})(-dirty)?\)' "$tmp/version-core.out"; then
  pass "mnemorphd --version names its commit"
else fail "mnemorphd --version names its commit: $(cat "$tmp/version-core.out")"; fi

run status "$bin/mnemorph" status --world "$world"
expect status 0 "mnemorph status --world <world>"
says status out "rc-smoke via local" "status reads the global config's model and provider"
says status out "world    $world" "status names the world"
if [ "$(uname -s)" = Linux ]; then
  if grep -E '^sandbox ' "$tmp/status.out" | grep -qv 'none'; then pass "status: the kernel guard is in force"
  else fail "status: the kernel guard is in force — $(grep -E '^sandbox ' "$tmp/status.out")"; fi
fi

run ask "$bin/mnemorph" run "make out.txt say fixed" --world "$world"
expect ask 0 "mnemorph run (headless, scripted provider)"
if [ "$(cat "$tmp/ask.out")" = "Fixed it." ]; then pass "the answer alone on stdout"
else fail "the answer alone on stdout: $(head -c 400 "$tmp/ask.out")"; fi
says ask err "write out.txt" "stderr shows the write"
says ask err "end gate PASS" "the end gate passed"
if [ "$(cat "$world/out.txt" 2>/dev/null)" = fixed ]; then pass "the act landed: out.txt says fixed"
else fail "the act landed: out.txt is '$(cat "$world/out.txt" 2>/dev/null)'"; fi

run ask-json "$bin/mnemorph" run --json "make out.txt say fixed" --world "$world"
expect ask-json 0 "mnemorph run --json"
if tail -n 1 "$tmp/ask-json.out" | python3 -c '
import json, sys
r = json.loads(sys.stdin.read())
assert r["type"] == "result" and r["answered"] is True and r["exit"] == 0, r
assert r["answer"] == "Fixed it.", r["answer"]
'; then pass "--json ends on an answered result line"; else fail "--json ends on an answered result line"; fi
requests=$(wc -l < "$tmp/requests.jsonl" | tr -d ' ')
if [ "$requests" -ge 4 ]; then pass "the provider was asked $requests times, on loopback only"
else fail "the provider was asked $requests times"; fi

run board-help "$bin/mnemorphd" board --help
expect board-help 0 "mnemorphd board --help"
says board-help out "board" "mnemorphd board --help names the board"

# ---- nothing touched that was not its own --------------------------------------------------------
section "the machine around it"
if [ "$(real_snapshot)" = "$real_before" ]; then pass "the real HOME's Mnemorph directories are untouched"
else fail "the real HOME's Mnemorph directories changed"; fi
if [ "$(foreign_snapshot)" = "$foreign_before" ]; then pass "other programs' config and state are untouched"
else fail "other programs' config and state changed"; diff <(echo "$foreign_before") <(foreign_snapshot) >&2 || true; fi
if [ -s "$home/.githooks.fired" ]; then fail "the user's global git hooks ran: $(tr '\n' ';' < "$home/.githooks.fired")"
else pass "the user's global git hooks never ran"; fi
commits_after=$(cd "$world" && in_home git rev-list --count HEAD)
head_after=$(cd "$world" && in_home git rev-parse HEAD)
if [ "$head_after" = "$head_before" ] && [ "$commits_after" = "$commits_before" ]; then
  pass "the world's history is as it was ($commits_after commits)"
else fail "the world's history changed: $commits_before -> $commits_after commits"; fi
if (cd "$world" && in_home git diff --quiet HEAD -- README.md src .gitignore); then pass "the world's tracked files are as they were"
else fail "the world's tracked files changed"; fi
if [ -d "$home/.local/state/mnemorph" ]; then pass "the harness keeps its state under XDG_STATE_HOME"
else fail "no state under XDG_STATE_HOME/mnemorph"; fi

echo
if [ "$fails" -eq 0 ]; then echo "rc-smoke: PASS"; else echo "rc-smoke: FAIL ($fails)" >&2; exit 1; fi
