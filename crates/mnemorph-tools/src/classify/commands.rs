//! One simple command, by name: wrappers unwrapped, then the table.

use mnemorph_races::OutwardKind::{
    Comment, Deploy, Fetch, Install, Label, Publish, PullRequest, PushBranch, PushTag, RerunCi,
    Rollback, WriteOutside,
};
use mnemorph_races::ToolClass::{Destructive, Outward, Write};
use mnemorph_races::{OutwardKind, ToolClass};

use super::paths::Act;
use super::words::Lit;
use super::{Kind, Rule, Tally, Walker, base_name};

/// Commands that only look.
const READS: &[&str] = &[
    "ls",
    "cat",
    "head",
    "tail",
    "less",
    "more",
    "wc",
    "grep",
    "egrep",
    "fgrep",
    "rg",
    "ag",
    "stat",
    "file",
    "du",
    "df",
    "pwd",
    "echo",
    "printf",
    "true",
    "false",
    "test",
    "[",
    "which",
    "type",
    "whereis",
    "diff",
    "cmp",
    "sort",
    "uniq",
    "cut",
    "tr",
    "jq",
    "yq",
    "awk",
    "gawk",
    "date",
    "uname",
    "whoami",
    "id",
    "hostname",
    "basename",
    "dirname",
    "realpath",
    "readlink",
    "tree",
    "column",
    "nl",
    "od",
    "xxd",
    "hexdump",
    "strings",
    "md5sum",
    "sha1sum",
    "sha256sum",
    "shasum",
    "base64",
    "seq",
    "sleep",
    "printenv",
    "export",
    "set",
    "unset",
    "alias",
    "read",
    "wait",
    "ps",
    "lsof",
    "top",
    "free",
    "uptime",
    "fd",
    "comm",
    "paste",
    "fold",
    "rev",
    "tac",
    "expr",
    "bc",
];

/// Commands that leave the machine whatever their arguments.
const NETWORK: &[&str] = &[
    "curl",
    "wget",
    "http",
    "https",
    "xh",
    "aria2c",
    "ssh",
    "scp",
    "sftp",
    "nc",
    "ncat",
    "netcat",
    "telnet",
    "ftp",
    "ping",
    "traceroute",
    "dig",
    "nslookup",
    "host",
    "whois",
    "psql",
    "mysql",
    "mongo",
    "mongosh",
    "redis-cli",
    "npx",
    "open",
    "xdg-open",
    "crontab",
    "systemctl",
    "launchctl",
];

/// Commands that fetch: what they print must not be run.
const FETCHERS: &[&str] = &[
    "curl", "wget", "http", "https", "xh", "aria2c", "nc", "ncat", "netcat",
];

/// Password managers and secret stores: never run by a body.
const SECRET_STORES: &[&str] = &[
    "pass",
    "bw",
    "bws",
    "lpass",
    "keepassxc-cli",
    "rbw",
    "nordpass",
    "su",
];

/// Disk and power tools: never run by a body.
const MACHINE: &[&str] = &[
    "fdisk", "sfdisk", "gdisk", "parted", "wipefs", "shutdown", "reboot", "halt", "poweroff",
];

/// Interpreters: they run arbitrary code, contained by the sandbox, judged as writes.
const INTERPRETERS: &[&str] = &[
    "python", "python3", "node", "deno", "bun", "ruby", "perl", "php", "lua", "Rscript",
];

const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh", "fish"];

/// Builtins that change what the commands after them run with: variables, options, aliases.
const STATE: &[&str] = &[
    "export",
    "declare",
    "typeset",
    "local",
    "readonly",
    "set",
    "unset",
    "alias",
    "shopt",
    "enable",
    "hash",
    "pushd",
    "popd",
    "read",
    "mapfile",
    "readarray",
    "getopts",
    "let",
];

/// Classify `argv` (the command name first), raising `t`.
pub(super) fn classify(w: &mut Walker<'_>, argv: &[Lit], t: &mut Tally) -> Kind {
    let Some(name) = argv[0].text.as_deref() else {
        t.refuse(
            Rule::NotLiteral,
            "the command name is known only at run time",
        );
        return Kind::default();
    };
    let name = base_name(name);
    let tail = &argv[1..];
    let words: Vec<Option<&str>> = tail.iter().map(|a| a.text.as_deref()).collect();
    match name {
        "sudo" | "doas" => {
            t.reach(
                OutwardKind::Other,
                Destructive,
                format!("`{name}` runs as root"),
            );
            let rest = skip_options(
                tail,
                &["-u", "-g", "-h", "-p", "-C", "-D", "-r", "-t", "-U"],
            );
            inner(w, rest, t)
        }
        "env" => {
            let mut rest = skip_options(tail, &["-u", "-C", "-S", "--unset", "--chdir"]);
            // Anything `env` sets, unsets or changes reaches the command it runs.
            w.tainted |= rest.len() < tail.len()
                || rest
                    .first()
                    .and_then(|a| a.text.as_deref())
                    .is_some_and(|s| s.contains('='));
            while rest
                .first()
                .and_then(|a| a.text.as_deref())
                .is_some_and(|s| s.contains('=') && !s.starts_with('='))
            {
                rest = &rest[1..];
            }
            inner(w, rest, t)
        }
        "nohup" | "time" | "exec" | "command" | "builtin" | "caffeinate" => {
            inner(w, skip_options(tail, &[]), t)
        }
        "nice" | "ionice" => inner(w, skip_options(tail, &["-n", "-c", "-p"]), t),
        "stdbuf" => inner(w, skip_options(tail, &["-i", "-o", "-e"]), t),
        "timeout" => {
            let rest = skip_options(tail, &["-s", "-k", "--signal", "--kill-after"]);
            inner(w, rest.get(1..).unwrap_or(&[]), t)
        }
        "watch" => inner(w, skip_options(tail, &["-n", "-d", "--interval"]), t),
        "xargs" => {
            let rest = skip_options(
                tail,
                &["-n", "-I", "-P", "-L", "-d", "-E", "-s", "-a", "-i", "-l"],
            );
            let mut fed = rest.to_vec();
            if !fed.is_empty() {
                fed.push(Lit::unknown());
            }
            inner(w, &fed, t)
        }
        "eval" => {
            match words.iter().copied().collect::<Option<Vec<_>>>() {
                Some(parts) => w.source(&parts.join(" "), t),
                None => t.refuse(
                    Rule::NotLiteral,
                    "`eval` runs a string known only at run time",
                ),
            }
            Kind::default()
        }
        "source" | "." => {
            w.tainted = true;
            t.raise(Write, format!("`{name}` runs a script"));
            Kind::default()
        }
        _ if STATE.contains(&name) || (name == "printf" && words.contains(&Some("-v"))) => {
            // What the shell carries to the commands after it: none of them is a post's act.
            w.tainted = true;
            table(w, name, tail, &words, t);
            Kind::default()
        }
        _ if SHELLS.contains(&name) => shell(w, name, tail, t),
        _ if INTERPRETERS.contains(&name) => interpreter(name, &words, t),
        _ => {
            table(w, name, tail, &words, t);
            Kind {
                fetches: FETCHERS.contains(&name),
                runs_stdin: false,
            }
        }
    }
}

/// The wrapped command, classified in full (the prefix rule already ran over the wrapper's words).
fn inner(w: &mut Walker<'_>, rest: &[Lit], t: &mut Tally) -> Kind {
    if rest.is_empty() {
        return Kind::default();
    }
    classify(w, rest, t)
}

/// Skip leading options; `with_value` names the ones that take the next word.
fn skip_options<'a>(args: &'a [Lit], with_value: &[&str]) -> &'a [Lit] {
    let mut i = 0;
    while let Some(Some(a)) = args.get(i).map(|a| a.text.as_deref()) {
        if a == "--" {
            return &args[i + 1..];
        }
        if !a.starts_with('-') || a == "-" {
            break;
        }
        i += if with_value.contains(&a) { 2 } else { 1 };
    }
    args.get(i..).unwrap_or(&[])
}

/// The words that are not options (after `--`, everything is an operand).
fn operands(args: &[Lit]) -> Vec<&Lit> {
    let mut out = Vec::new();
    let mut ended = false;
    for a in args {
        match a.text.as_deref() {
            Some("--") if !ended => ended = true,
            Some(s) if !ended && s.starts_with('-') && s != "-" => {}
            _ => out.push(a),
        }
    }
    out
}

fn has_short(words: &[Option<&str>], flag: char) -> bool {
    words
        .iter()
        .flatten()
        .any(|w| w.starts_with('-') && !w.starts_with("--") && w[1..].contains(flag))
}

fn has(words: &[Option<&str>], wanted: &[&str]) -> bool {
    words.iter().flatten().any(|w| {
        wanted
            .iter()
            .any(|x| w == x || w.starts_with(&format!("{x}=")))
    })
}

fn shell(w: &mut Walker<'_>, name: &str, args: &[Lit], t: &mut Tally) -> Kind {
    let flag_c = args.iter().position(|a| {
        a.text
            .as_deref()
            .is_some_and(|s| s.starts_with('-') && !s.starts_with("--") && s[1..].contains('c'))
    });
    if let Some(i) = flag_c {
        match args.get(i + 1).map(|a| a.text.as_deref()) {
            Some(Some(src)) => {
                let saved = w.cwd.clone();
                w.source(src, t);
                w.cwd = saved;
            }
            Some(None) => t.refuse(
                Rule::NotLiteral,
                format!("`{name} -c` runs a string known only at run time"),
            ),
            None => {}
        }
        return Kind::default();
    }
    t.raise(Write, format!("`{name}` runs a script"));
    let script = operands(args).into_iter().next();
    Kind {
        fetches: false,
        runs_stdin: script.is_none(),
    }
}

fn interpreter(name: &str, words: &[Option<&str>], t: &mut Tally) -> Kind {
    if name == "perl" && has_short(words, 'i') {
        t.refuse(
            Rule::InPlaceEdit,
            "in-place edits run blind (`perl -i`): use the edit or multiedit tool",
        );
    }
    t.raise(Write, format!("`{name}` runs code"));
    let code_flag = words
        .iter()
        .flatten()
        .any(|w| matches!(*w, "-c" | "-e" | "-E" | "-m" | "--eval" | "-r"));
    let script = words.iter().flatten().any(|w| !w.starts_with('-'));
    Kind {
        fetches: false,
        runs_stdin: !code_flag && !script,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per command family: the table reads best whole"
)]
fn table(w: &mut Walker<'_>, name: &str, args: &[Lit], words: &[Option<&str>], t: &mut Tally) {
    let sub = operands(args)
        .first()
        .and_then(|a| a.text.as_deref())
        .unwrap_or("");
    match name {
        "git" => git(w, args, t),
        "gh" => gh(words, t),
        "cargo" => match sub {
            "publish" | "yank" | "owner" => t.reach(
                Publish,
                Destructive,
                format!("`cargo {sub}` publishes to a registry"),
            ),
            "install" | "update" | "add" | "generate-lockfile" => {
                t.reach(
                    Install,
                    Outward,
                    format!("`cargo {sub}` reaches the registry"),
                );
            }
            "fetch" | "search" => {
                t.reach(
                    Fetch,
                    Outward,
                    format!("`cargo {sub}` reaches the registry"),
                );
            }
            "login" | "logout" => {
                t.raise(Outward, format!("`cargo {sub}` reaches the registry"));
            }
            _ => t.raise(Write, "`cargo` builds and runs code"),
        },
        "npm" | "pnpm" | "yarn" => match sub {
            "publish" | "unpublish" | "deprecate" | "dist-tag" | "owner" => {
                t.reach(
                    Publish,
                    Destructive,
                    format!("`{name} {sub}` publishes to a registry"),
                );
            }
            "install" | "i" | "add" | "update" | "up" | "upgrade" | "ci" | "dlx" => {
                t.reach(
                    Install,
                    Outward,
                    format!("`{name} {sub}` reaches the registry"),
                );
            }
            "whoami" | "view" | "info" | "outdated" | "audit" => {
                t.reach(
                    Fetch,
                    Outward,
                    format!("`{name} {sub}` reaches the registry"),
                );
            }
            "login" | "logout" => {
                t.raise(Outward, format!("`{name} {sub}` reaches the registry"));
            }
            _ => t.raise(Write, format!("`{name}` runs scripts")),
        },
        "pip" | "pip3" | "uv" | "poetry" | "pipx" => match sub {
            "upload" | "publish" => t.reach(
                Publish,
                Destructive,
                format!("`{name} {sub}` publishes to a registry"),
            ),
            "install" | "add" | "sync" | "lock" | "update" => {
                t.reach(
                    Install,
                    Outward,
                    format!("`{name} {sub}` reaches the registry"),
                );
            }
            "download" | "search" => {
                t.reach(
                    Fetch,
                    Outward,
                    format!("`{name} {sub}` reaches the registry"),
                );
            }
            _ => t.raise(Write, format!("`{name}` runs code")),
        },
        "twine" => t.reach(Publish, Destructive, "`twine` publishes to a registry"),
        "go" => match sub {
            "get" | "install" => {
                t.reach(Install, Outward, format!("`go {sub}` reaches the network"));
            }
            "mod"
                if words
                    .iter()
                    .flatten()
                    .any(|w| *w == "download" || *w == "tidy") =>
            {
                t.reach(Install, Outward, "`go mod` reaches the network");
            }
            _ => t.raise(Write, "`go` builds and runs code"),
        },
        "docker" | "podman" => match sub {
            "push" => t.reach(
                Publish,
                Destructive,
                format!("`{name} push` publishes an image"),
            ),
            "pull" | "search" => {
                t.reach(Fetch, Outward, format!("`{name} {sub}` reads a registry"));
            }
            "rm" | "rmi" | "prune" | "kill" => {
                t.reach(
                    WriteOutside,
                    Destructive,
                    format!("`{name} {sub}` deletes outside the world"),
                );
            }
            "system" | "volume" | "image" | "container" | "network" | "builder"
                if words.iter().flatten().any(|w| matches!(*w, "prune" | "rm")) =>
            {
                t.reach(
                    WriteOutside,
                    Destructive,
                    format!("`{name} {sub}` deletes outside the world"),
                );
            }
            "ps" | "images" | "inspect" | "logs" | "version" | "info" => {}
            _ => t.raise(Outward, format!("`{name}` reaches beyond the world")),
        },
        "brew" | "apt" | "apt-get" | "dnf" | "yum" | "pacman" | "port" => match sub {
            "uninstall" | "remove" | "rm" | "purge" | "autoremove" | "cleanup" | "-R" | "-Rs" => {
                t.reach(
                    Install,
                    Destructive,
                    format!("`{name} {sub}` deletes outside the world"),
                );
            }
            "list" | "info" | "search" | "show" | "--version" | "-Q" => {}
            _ => t.reach(
                Install,
                Outward,
                format!("`{name}` installs outside the world"),
            ),
        },
        "rsync" => {
            if words.iter().flatten().any(|w| remote_spec(w)) {
                t.raise(Outward, "`rsync` reaches another host");
            }
            copy(w, args, Act::Read, t);
        }
        "op" => match words.first().copied().flatten().unwrap_or("") {
            "" | "--help" | "-h" => {}
            "--version" | "-v" | "whoami" | "signout" | "account" => {
                t.raise(Outward, "`op` reaches 1Password");
            }
            _ => t.refuse(Rule::SecretStore, "`op` reads secrets from 1Password"),
        },
        "security" => {
            if matches!(
                sub,
                "dump-keychain" | "find-generic-password" | "find-internet-password" | "export"
            ) {
                t.refuse(
                    Rule::SecretStore,
                    format!("`security {sub}` reads the keychain"),
                );
            }
        }
        "gpg" | "gpg2" => {
            if words
                .iter()
                .flatten()
                .any(|w| w.starts_with("--export-secret") || matches!(*w, "--decrypt" | "-d"))
            {
                t.refuse(Rule::SecretStore, "`gpg` exports or decrypts secrets");
            }
        }
        "diskutil" => {
            if [
                "erase",
                "partition",
                "zero",
                "secureerase",
                "reformat",
                "randomdisk",
            ]
            .iter()
            .any(|p| sub.to_ascii_lowercase().starts_with(p))
            {
                t.refuse(
                    Rule::MachineTool,
                    format!("`diskutil {sub}` rewrites a disk"),
                );
            }
        }
        "sed" => {
            let in_place = has_short(words, 'i') || has(words, &["--in-place"]);
            if in_place {
                t.refuse(
                    Rule::InPlaceEdit,
                    "in-place edits run blind (`sed -i`): use the edit or multiedit tool",
                );
            }
            // In place, the files are written: the class an approval would run at says so.
            let act = if in_place { Act::Modify } else { Act::Read };
            for a in operands(args).into_iter().skip(1) {
                w.touch(a, act, "`sed`", t);
            }
        }
        "touch" | "mkdir" | "tee" | "truncate" => {
            for a in operands(args) {
                w.touch(a, Act::Modify, &format!("`{name}`"), t);
            }
        }
        "rm" | "rmdir" | "unlink" => {
            for a in operands(args) {
                w.touch(a, Act::Delete, &format!("`{name}`"), t);
            }
        }
        "shred" => {
            t.raise(Destructive, "`shred` destroys data");
            for a in operands(args) {
                w.touch(a, Act::Delete, "`shred`", t);
            }
        }
        "chmod" | "chown" | "chgrp" => {
            let targets = operands(args);
            let skip = usize::from(!has(words, &["--reference"]));
            for a in targets.into_iter().skip(skip) {
                w.touch(a, Act::Modify, &format!("`{name}`"), t);
            }
        }
        "cp" | "ln" | "install" => copy(w, args, Act::Read, t),
        "mv" => copy(w, args, Act::Delete, t),
        "dd" => {
            for word in words.iter().flatten() {
                if let Some(p) = word.strip_prefix("if=") {
                    w.touch(&lit(p), Act::Read, "`dd`", t);
                } else if let Some(p) = word.strip_prefix("of=") {
                    w.touch(&lit(p), Act::Modify, "`dd`", t);
                }
            }
        }
        "find" => find(w, args, t),
        "kubectl" => kubectl(sub, words, t),
        "helm" => helm(sub, t),
        "kill" | "pkill" | "killall" => t.raise(Write, format!("`{name}` signals processes")),
        "make" | "just" | "cmake" | "ninja" | "rustc" | "gcc" | "cc" | "clang" | "tsc"
        | "vitest" | "jest" | "pytest" | "biome" | "rustfmt" | "prettier" | "eslint" => {
            t.raise(Write, format!("`{name}` builds or runs code"));
        }
        _ if NETWORK.contains(&name) => {
            let kind = if reads_only(name, words) {
                Fetch
            } else {
                OutwardKind::Other
            };
            t.reach(kind, Outward, format!("`{name}` reaches the network"));
        }
        _ if SECRET_STORES.contains(&name) => {
            t.refuse(Rule::SecretStore, format!("`{name}` reads secrets"));
        }
        _ if MACHINE.contains(&name) || name.starts_with("mkfs") => {
            t.refuse(
                Rule::MachineTool,
                format!("`{name}` rewrites disks or stops the machine"),
            );
        }
        _ if READS.contains(&name) => {
            for a in operands(args) {
                w.touch(a, Act::Read, &format!("`{name}`"), t);
            }
        }
        _ => t.raise(Write, format!("`{name}` is not in the read table")),
    }
}

/// `cp`, `ln`, `install`, `mv`, `rsync`: the last operand is written, the others read (or, for
/// `mv`, deleted from where they were).
fn copy(w: &mut Walker<'_>, args: &[Lit], sources: Act, t: &mut Tally) {
    let ops = operands(args);
    if let Some((dest, srcs)) = ops.split_last() {
        for s in srcs {
            w.touch(s, sources, "a copy or move", t);
        }
        if !srcs.is_empty() {
            w.touch(dest, Act::Modify, "a copy or move", t);
        }
    }
}

fn find(w: &mut Walker<'_>, args: &[Lit], t: &mut Tally) {
    let starts: Vec<&Lit> = args
        .iter()
        .take_while(|a| {
            a.text
                .as_deref()
                .is_none_or(|s| !s.starts_with(['-', '(', '!']))
        })
        .collect();
    for s in &starts {
        w.touch(s, Act::Read, "`find`", t);
    }
    // What find acts on is what it finds: a child of each start, never the start itself.
    let found: Vec<Lit> = if starts.is_empty() {
        vec![lit("./found")]
    } else {
        starts
            .iter()
            .map(|s| {
                s.text.as_deref().map_or_else(Lit::unknown, |t| {
                    lit(&format!("{}/found", t.trim_end_matches('/')))
                })
            })
            .collect()
    };
    let rest = &args[starts.len()..];
    let mut i = 0;
    while i < rest.len() {
        match rest[i].text.as_deref() {
            Some("-delete") => {
                for found in &found {
                    w.touch(found, Act::Delete, "`find -delete`", t);
                }
            }
            Some("-exec" | "-execdir" | "-ok" | "-okdir") => {
                let end = rest[i + 1..]
                    .iter()
                    .position(|a| matches!(a.text.as_deref(), Some(";" | "+")))
                    .map_or(rest.len(), |e| i + 1 + e);
                let cmd: Vec<Lit> = rest[i + 1..end]
                    .iter()
                    .map(|a| {
                        if a.text.as_deref() == Some("{}") {
                            found[0].clone()
                        } else {
                            a.clone()
                        }
                    })
                    .collect();
                if !cmd.is_empty() {
                    let mut part = Tally::default();
                    for k in 1..=cmd.len() {
                        classify(w, &cmd[..k], &mut part);
                    }
                    t.absorb(part);
                }
                i = end;
            }
            Some("-fprint" | "-fprint0" | "-fprintf" | "-fls") => {
                if let Some(target) = rest.get(i + 1) {
                    w.touch(target, Act::Modify, "`find -fprint`", t);
                }
            }
            _ => {}
        }
        i += 1;
    }
}

fn lit(s: &str) -> Lit {
    Lit {
        text: Some(s.to_owned()),
        subs: Vec::new(),
    }
}

/// `host:path` or `user@host:path`, as rsync and scp name another machine.
fn remote_spec(word: &str) -> bool {
    !word.starts_with(['-', '/', '.'])
        && word
            .split_once(':')
            .is_some_and(|(host, _)| !host.is_empty() && !host.contains('/'))
}

/// git: global options skipped, then the subcommand.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per git subcommand: the table reads best whole"
)]
fn git(w: &mut Walker<'_>, args: &[Lit], t: &mut Tally) {
    let mut i = 0;
    // A global option other than `-C` into the world changes how git runs or which repository it
    // reads; `-c` and `--config-env` set its config outright.
    let mut global = false;
    let mut configured = false;
    while let Some(Some(a)) = args.get(i).map(|a| a.text.as_deref()) {
        if !a.starts_with('-') {
            break;
        }
        configured |= a == "-c" || a.starts_with("--config-env");
        let inside = a == "-C"
            && args.get(i + 1).is_some_and(|d| {
                matches!(
                    super::paths::locate(d.text.as_deref(), w.cwd.as_deref(), w.place),
                    super::paths::Spot::Inside { .. }
                )
            });
        global |= !inside;
        i += if matches!(
            a,
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--exec-path"
        ) {
            2
        } else {
            1
        };
    }
    let Some(sub_lit) = args.get(i) else { return };
    let Some(sub) = sub_lit.text.as_deref() else {
        t.refuse(
            Rule::NotLiteral,
            "the git subcommand is known only at run time",
        );
        return;
    };
    let rest = &args[i + 1..];
    let words: Vec<&str> = rest.iter().filter_map(|a| a.text.as_deref()).collect();
    let texts: Vec<Option<&str>> = rest.iter().map(|a| a.text.as_deref()).collect();
    // An outward git act is a post's only as its words alone say it: no global option (`-c`,
    // `-C`, `--git-dir`, `--config-env`), no word known only at run time, nothing earlier in the
    // command that changed how git runs.
    let plain = !global && !w.tainted && texts.iter().all(Option::is_some);
    // Written config (`git config`, `git remote set-url`, an upstream) reaches every git after it.
    let writes_config = match sub {
        "config" => !words.iter().any(|w| {
            matches!(
                *w,
                "--get" | "--get-all" | "--get-regexp" | "-l" | "--list" | "get" | "list"
            )
        }),
        "remote" => words.iter().any(|w| {
            matches!(
                *w,
                "add" | "rename" | "remove" | "rm" | "set-head" | "set-branches" | "set-url"
            )
        }),
        "branch" => words
            .iter()
            .any(|w| w.starts_with("--set-upstream") || *w == "-u" || *w == "--unset-upstream"),
        _ => false,
    };
    w.tainted |= writes_config;
    let any = |xs: &[&str]| {
        words
            .iter()
            .any(|w| xs.iter().any(|x| w == x || w.starts_with(&format!("{x}="))))
    };
    let short = |c: char| {
        words
            .iter()
            .any(|w| w.starts_with('-') && !w.starts_with("--") && w[1..].contains(c))
    };
    match sub {
        "status" | "log" | "diff" | "show" | "blame" | "rev-parse" | "ls-files" | "ls-tree"
        | "cat-file" | "describe" | "shortlog" | "grep" | "rev-list" | "help" | "version"
        | "whatchanged" | "range-diff" | "merge-base" | "for-each-ref" | "show-ref"
        | "check-ignore" | "count-objects" => {}
        "push" => {
            if configured {
                t.reach(
                    OutwardKind::Other,
                    Destructive,
                    "`git -c` sets the config the push runs with (`core.sshCommand`, a URL)",
                );
            }
            push(&words, plain, t);
        }
        "fetch" | "pull" | "clone" | "ls-remote" => {
            t.reach(
                fetch_kind(&texts, plain),
                Outward,
                format!("`git {sub}` reaches a remote"),
            );
        }
        "archive" if any(&["--remote"]) => {
            t.raise(Outward, "`git archive --remote` reaches a remote");
        }
        "send-email" | "request-pull" => {
            t.raise(Outward, format!("`git {sub}` reaches a remote"));
        }
        "remote" => match words.first().copied() {
            None | Some("-v" | "get-url") => {}
            Some("update" | "show" | "prune") => {
                let kind = if plain
                    && words[1..]
                        .iter()
                        .all(|w| matches!(*w, "-v" | "-p" | "--prune" | "-n") || remote_ok(w))
                {
                    Fetch
                } else {
                    OutwardKind::Other
                };
                t.reach(kind, Outward, "`git remote` reaches a remote");
            }
            Some(_) => t.raise(Write, "`git remote` changes the repository's config"),
        },
        "submodule"
            if words
                .iter()
                .any(|w| matches!(*w, "update" | "sync" | "add")) =>
        {
            // A submodule's URL and update command come from config: no post's act.
            t.raise(Outward, "`git submodule` reaches a remote");
        }
        "reset" if any(&["--hard", "--merge", "--keep"]) => {
            t.raise(Destructive, "`git reset --hard` discards work");
        }
        "rebase" | "filter-branch" | "filter-repo" | "replace" => {
            t.raise(Destructive, format!("`git {sub}` rewrites history"));
        }
        "commit" if any(&["--amend"]) => {
            t.raise(Destructive, "`git commit --amend` rewrites history");
        }
        "clean" if short('f') || any(&["--force"]) => {
            t.raise(Destructive, "`git clean -f` deletes untracked work");
        }
        "branch"
            if short('D')
                || (short('d') && short('f'))
                || any(&["--delete"]) && any(&["--force"]) =>
        {
            t.raise(Destructive, "a forced branch delete loses commits");
        }
        "branch"
            if short('d')
                || short('m')
                || short('M')
                || short('c')
                || words.iter().any(|w| !w.starts_with('-')) =>
        {
            t.raise(Write, "`git branch` changes refs");
        }
        "stash" if matches!(words.first(), Some(&("list" | "show"))) => {}
        "branch" | "tag" | "reflog" | "worktree" | "config" | "notes"
            if words.is_empty()
                || words.iter().all(|w| {
                    matches!(
                        *w,
                        "-l" | "--list"
                            | "-a"
                            | "-r"
                            | "-v"
                            | "-vv"
                            | "list"
                            | "show"
                            | "--get"
                            | "--get-all"
                            | "--show-origin"
                    )
                }) => {}
        "reflog" if any(&["expire", "delete"]) => {
            let now = words.iter().any(|w| {
                w.strip_prefix("--expire=")
                    .or_else(|| w.strip_prefix("--expire-unreachable="))
                    .is_some_and(|v| matches!(v, "now" | "all" | "0"))
            });
            if now || words.contains(&"delete") {
                t.raise(Destructive, "expiring the reflog now loses the way back");
            } else {
                t.raise(Write, "`git reflog expire` prunes old entries");
            }
        }
        "gc" | "prune" => {
            let now = words.iter().any(|w| {
                w.strip_prefix("--prune=")
                    .or_else(|| w.strip_prefix("--expire="))
                    .is_some_and(|v| matches!(v, "now" | "all"))
            });
            if now {
                t.raise(Destructive, "pruning now loses unreachable commits");
            } else {
                t.raise(Write, format!("`git {sub}` compacts the repository"));
            }
        }
        "update-ref" if short('d') => t.raise(Destructive, "`git update-ref -d` deletes a ref"),
        "tag" if short('d') || any(&["--delete"]) => {
            t.raise(Write, "`git tag -d` deletes a local tag");
        }
        "rm" | "mv" => {
            t.raise(Write, format!("`git {sub}` changes the world"));
            for a in operands(rest) {
                w.touch(
                    a,
                    if sub == "rm" {
                        Act::Delete
                    } else {
                        Act::Modify
                    },
                    &format!("`git {sub}`"),
                    t,
                );
            }
        }
        _ => t.raise(Write, format!("`git {sub}` changes the repository")),
    }
}

/// gh: everything reaches GitHub; deletes, visibility, releases and secrets are worse.
fn gh(words: &[Option<&str>], t: &mut Tally) {
    let ws: Vec<&str> = words.iter().flatten().copied().collect();
    let pair = (
        ws.first().copied().unwrap_or(""),
        ws.get(1).copied().unwrap_or(""),
    );
    if pair == ("auth", "token") {
        t.refuse(Rule::SecretStore, "`gh auth token` prints a credential");
        return;
    }
    let destructive = match pair {
        (_, "delete")
        | ("release", "create" | "upload" | "edit")
        | ("repo", "archive" | "rename" | "transfer")
        | ("secret" | "variable", "set" | "remove") => true,
        ("repo", "edit") => ws.iter().any(|w| w.starts_with("--visibility")),
        ("api", _) => api_writes(words),
        _ => false,
    };
    let class = if destructive { Destructive } else { Outward };
    let why = if destructive {
        format!(
            "`gh {} {}` deletes, publishes or changes what others see",
            pair.0, pair.1
        )
    } else {
        "`gh` reaches GitHub".to_owned()
    };
    for kind in gh_acts(pair, &ws, words) {
        t.reach(kind, class, why.clone());
    }
}

/// The kinds of outward act a `gh` command does: an edit names a label, a pull request, or
/// both; a subcommand not named here is [`OutwardKind::Other`].
fn gh_acts(pair: (&str, &str), ws: &[&str], words: &[Option<&str>]) -> Vec<OutwardKind> {
    const LABELS: &[&str] = &["--add-label", "--remove-label", "--label", "-l"];
    const REPO: &[&str] = &["-R", "--repo"];
    // An edit's flags: labels, the repository, and what else it changes.
    let mut labels = false;
    let mut other = false;
    let mut flags = ws.iter().skip(2);
    while let Some(w) = flags.next() {
        let bare = w.split_once('=').map_or(*w, |(f, _)| f);
        if !w.starts_with('-') {
            continue;
        }
        if LABELS.contains(&bare) {
            labels = true;
        } else if !REPO.contains(&bare) {
            other = true;
        }
        if bare == *w && (LABELS.contains(&bare) || REPO.contains(&bare)) {
            flags.next();
        }
    }
    // A subcommand known only at run time could be any of them.
    if words.iter().take(2).any(Option::is_none) {
        return vec![OutwardKind::Other];
    }
    let unknown = words.iter().any(Option::is_none);
    match pair {
        ("pr", "create" | "edit" | "ready" | "reopen" | "close") => {
            let mut a = vec![PullRequest];
            if labels {
                a.push(Label);
            }
            a
        }
        ("issue", "edit") => {
            let mut a = Vec::new();
            if labels {
                a.push(Label);
            }
            if other || unknown || !labels {
                a.push(OutwardKind::Other);
            }
            a
        }
        ("pr" | "issue", "comment") => vec![Comment],
        ("label", "create" | "edit" | "delete" | "clone") => vec![Label],
        ("run", "rerun") => vec![RerunCi],
        ("release", "create" | "upload" | "edit" | "delete") => vec![Publish],
        ("api", _) if !unknown && !api_writes(words) => vec![Fetch],
        ("search", _)
        | ("pr", "checkout")
        | ("repo", "clone")
        | (
            "pr" | "issue" | "run" | "workflow" | "release" | "repo" | "label" | "auth" | "cache"
            | "ruleset" | "secret" | "variable",
            "list" | "view" | "status" | "diff" | "checks" | "watch" | "download",
        ) => vec![Fetch],
        _ => vec![OutwardKind::Other],
    }
}

/// The `git push` options that change nothing it pushes.
const PUSH_QUIET: &[&str] = &[
    "-u",
    "--set-upstream",
    "-q",
    "--quiet",
    "-v",
    "--verbose",
    "--progress",
    "--no-progress",
    "--porcelain",
    "-n",
    "--dry-run",
    "--atomic",
    "--no-verify",
];

/// `git push`: outward only as one plain branch push. A force of any kind but the create-only
/// lease, a delete, a mirror, every branch or tag at once, a tag or a ref outside `refs/heads/`, a
/// branch pushed onto another name, more than one refspec, a helper program or a push option on
/// the remote: destructive. An option it does not know is destructive too, so a word only ever
/// raises the class.
fn push(words: &[&str], plain: bool, t: &mut Tally) {
    let mut why: Option<String> = None;
    let mut positional = Vec::new();
    for w in words {
        if let Some(lease) = w.strip_prefix("--force-with-lease=") {
            // `<ref>:` with nothing expected: the ref must not exist, so nothing is overwritten.
            let create_only = lease
                .strip_suffix(':')
                .is_some_and(|r| r.starts_with("refs/heads/") && !r.contains(':'));
            if !create_only {
                why.get_or_insert_with(|| format!("`{w}` may overwrite a branch"));
            }
        } else if let Some(flags) = w.strip_prefix('-') {
            let short_quiet = !flags.starts_with('-')
                && !flags.is_empty()
                && flags.chars().all(|c| matches!(c, 'u' | 'q' | 'v' | 'n'));
            if !PUSH_QUIET.contains(w) && !short_quiet {
                why.get_or_insert_with(|| {
                    format!("`git push {w}` forces, deletes, mirrors or reaches past a branch")
                });
            }
        } else {
            positional.push(*w);
        }
    }
    let refspecs = positional.get(1..).unwrap_or_default();
    if refspecs.len() > 1 {
        why.get_or_insert_with(|| "a push of more than one refspec".to_owned());
    }
    if let Some(spec) = refspecs.first()
        && let Some(reason) = refspec(spec)
    {
        why.get_or_insert(reason);
    }
    // A branch push is the forge's only when it cannot mean more than it says: one remote, one
    // `refs/heads/` refspec, nothing in the command that changes how git runs.
    let branch =
        matches!(positional.as_slice(), [remote, spec] if remote_ok(remote) && branch_spec(spec));
    match why {
        Some(reason) if plain && tags_only(words) => t.reach(PushTag, Destructive, reason),
        // A forced, deleting or mirroring push is held by no post.
        Some(reason) => t.reach(OutwardKind::Other, Destructive, reason),
        None if plain && branch => {
            t.reach(PushBranch, Outward, "`git push` leaves the machine");
        }
        None => t.reach(
            OutwardKind::Other,
            Outward,
            "`git push` whose refs or remote its words do not name alone",
        ),
    }
}

/// A ref name with nothing a refspec reads as more: no glob, force, range or second ref.
fn ref_name(r: &str) -> bool {
    !r.is_empty()
        && !r.starts_with('-')
        && !r.starts_with("refs/")
        && !r.contains(['*', '+', ':', '^', '~', '?', '[', '\\', ' '])
        && !r.contains("..")
}

/// A remote named plainly: no transport helper (`ext::`, `fd::`), no option.
fn remote_ok(remote: &str) -> bool {
    !remote.is_empty()
        && !remote.starts_with('-')
        && !remote.contains("::")
        && !remote.starts_with("ext:")
        && !remote.starts_with("fd:")
}

/// `refs/heads/X`, `HEAD:refs/heads/X` or `X:refs/heads/X`: one branch, named in full.
fn branch_spec(spec: &str) -> bool {
    if let Some(b) = spec.strip_prefix("refs/heads/") {
        return ref_name(b);
    }
    spec.split_once(':').is_some_and(|(src, dst)| {
        dst.strip_prefix("refs/heads/")
            .is_some_and(|b| ref_name(b) && (src == "HEAD" || src == b))
    })
}

/// Whether a `git push` pushes one tag and nothing else: `tag <name>` or `refs/tags/<name>` to
/// a plain remote, with no force, delete or mirror.
fn tags_only(words: &[&str]) -> bool {
    if words
        .iter()
        .any(|w| w.starts_with('-') && !PUSH_QUIET.contains(w))
    {
        return false;
    }
    let positional: Vec<&str> = words
        .iter()
        .filter(|w| !w.starts_with('-'))
        .copied()
        .collect();
    match positional.as_slice() {
        [remote, "tag", t] => remote_ok(remote) && ref_name(t),
        [remote, r] => remote_ok(remote) && r.strip_prefix("refs/tags/").is_some_and(ref_name),
        _ => false,
    }
}

/// A command's options, read against an allowlist: `flags` stand alone, `values` take one (as
/// `--x=v`, `--x v`, `-xv` or `-x v`). Every option must be on it, and every operand pass
/// `operand`.
fn allowed(
    words: &[Option<&str>],
    flags: &[&str],
    values: &[&str],
    operand: impl Fn(&str) -> bool,
) -> bool {
    let mut it = words.iter();
    while let Some(w) = it.next() {
        let Some(w) = *w else { return false };
        if let Some(long) = w.strip_prefix("--") {
            if long.is_empty() {
                return false;
            }
            let (name, has_value) = match w.split_once('=') {
                Some((n, _)) => (n, true),
                None => (w, false),
            };
            if values.contains(&name) {
                if !has_value && it.next().is_none() {
                    return false;
                }
            } else if has_value || !flags.contains(&name) {
                return false;
            }
        } else if let Some(short) = w.strip_prefix('-').filter(|s| !s.is_empty()) {
            for (k, c) in short.char_indices() {
                let flag = format!("-{c}");
                if values.contains(&flag.as_str()) {
                    if short[k + c.len_utf8()..].is_empty() && it.next().is_none() {
                        return false;
                    }
                    break;
                }
                if !flags.contains(&flag.as_str()) {
                    return false;
                }
            }
        } else if !operand(w) {
            return false;
        }
    }
    true
}

/// A URL a fetch may name: http(s), or no scheme at all (read as http).
fn web_url(w: &str) -> bool {
    !w.contains("://") || w.starts_with("http://") || w.starts_with("https://")
}

/// Whether a network command only reads: a lookup, or a plain GET whose every option is on the
/// command's allowlist.
fn reads_only(name: &str, words: &[Option<&str>]) -> bool {
    match name {
        "ping" | "traceroute" | "dig" | "nslookup" | "host" | "whois" => {
            words.iter().all(Option::is_some)
        }
        "curl" => {
            // `-X` is allowed only as a GET or a HEAD.
            let mut ws = words.to_vec();
            if let Some(i) = ws
                .iter()
                .position(|w| matches!(*w, Some("-X" | "--request")))
            {
                if !matches!(ws.get(i + 1), Some(Some("GET" | "HEAD"))) {
                    return false;
                }
                ws.drain(i..=i + 1);
            }
            allowed(
                &ws,
                &[
                    "-s",
                    "--silent",
                    "-S",
                    "--show-error",
                    "-f",
                    "--fail",
                    "--fail-with-body",
                    "-L",
                    "--location",
                    "-I",
                    "--head",
                    "-i",
                    "--include",
                    "-v",
                    "--verbose",
                    "--compressed",
                    "-4",
                    "-6",
                    "-N",
                    "--no-buffer",
                    "-O",
                    "--remote-name",
                    "-J",
                    "--remote-header-name",
                    "--create-dirs",
                    "--http1.1",
                    "--http2",
                ],
                &[
                    "-o",
                    "--output",
                    "-m",
                    "--max-time",
                    "--connect-timeout",
                    "--retry",
                    "--retry-delay",
                    "--max-redirs",
                    "-A",
                    "--user-agent",
                    "-r",
                    "--range",
                ],
                web_url,
            )
        }
        "wget" => allowed(
            words,
            &[
                "-q",
                "--quiet",
                "-c",
                "--continue",
                "-N",
                "--timestamping",
                "-S",
                "--server-response",
                "--spider",
                "--no-verbose",
                "--https-only",
            ],
            &[
                "-O",
                "--output-document",
                "-P",
                "--directory-prefix",
                "-T",
                "--timeout",
                "-t",
                "--tries",
            ],
            web_url,
        ),
        "aria2c" => allowed(
            words,
            &["-q", "--quiet", "-c", "--continue"],
            &[
                "-d",
                "--dir",
                "-o",
                "--out",
                "-x",
                "--max-connection-per-server",
                "-s",
                "--split",
            ],
            web_url,
        ),
        "http" | "https" | "xh" => match words {
            [Some(url)] => !url.starts_with('-'),
            [Some(method), Some(url)] => matches!(*method, "GET" | "HEAD") && !url.starts_with('-'),
            _ => false,
        },
        _ => false,
    }
}

/// `git fetch`, `pull`, `clone`, `ls-remote`: a fetch only with allowlisted options and plain
/// remotes — no `--upload-pack`, no config, no transport helper — and nothing that changes how
/// git runs; anything else is no post's.
fn fetch_kind(words: &[Option<&str>], plain: bool) -> OutwardKind {
    let ok = plain
        && allowed(
            words,
            &[
                "-q",
                "--quiet",
                "-v",
                "--verbose",
                "--prune",
                "-p",
                "--prune-tags",
                "--tags",
                "-t",
                "--no-tags",
                "--all",
                "--unshallow",
                "--single-branch",
                "--no-single-branch",
                "--bare",
                "--no-checkout",
                "--ff-only",
                "--no-rebase",
                "--heads",
                "--refs",
                "--progress",
                "--no-progress",
            ],
            &["--depth", "--shallow-since", "-b", "--branch", "--filter"],
            remote_ok,
        );
    if ok { Fetch } else { OutwardKind::Other }
}

/// `kubectl`: reads are a fetch, a rollout undo a rollback, what changes the cluster a deploy;
/// a shell into it, a copy or a tunnel is another act.
fn kubectl(sub: &str, words: &[Option<&str>], t: &mut Tally) {
    let rollout = |w: &str| words.iter().flatten().any(|x| *x == w);
    // A secret's resource, however named (`secret`, `secrets/db`, `pods,secrets`), is read only
    // by refusal: a cluster's secrets are a secret store.
    let secret = words.iter().flatten().any(|w| {
        w.split(',').any(|r| {
            let kind = r.split('/').next().unwrap_or(r);
            matches!(kind.split('.').next().unwrap_or(kind), "secret" | "secrets")
        })
    });
    if secret {
        t.refuse(
            Rule::SecretStore,
            format!("`kubectl {sub}` names a cluster's secrets"),
        );
    }
    let (kind, class): (OutwardKind, ToolClass) = match sub {
        "get" | "describe" | "logs" | "top" | "version" | "explain" | "api-resources"
        | "api-versions" | "cluster-info" | "diff" | "events" => (Fetch, Outward),
        "rollout" if rollout("undo") => (Rollback, Outward),
        "rollout" if rollout("status") || rollout("history") => (Fetch, Outward),
        "apply" | "create" | "replace" | "patch" | "set" | "scale" | "autoscale" | "expose"
        | "label" | "annotate" | "rollout" => (Deploy, Outward),
        "delete" => (Deploy, Destructive),
        _ => (OutwardKind::Other, Outward),
    };
    t.reach(kind, class, format!("`kubectl {sub}` reaches a cluster"));
}

/// `helm`: an install or upgrade deploys, a rollback rolls back, an uninstall removes a
/// deployment, a push publishes a chart; reads are a fetch.
fn helm(sub: &str, t: &mut Tally) {
    let (kind, class) = match sub {
        "install" | "upgrade" => (Deploy, Outward),
        "rollback" => (Rollback, Outward),
        "uninstall" | "delete" => (Deploy, Destructive),
        "push" => (Publish, Destructive),
        "status" | "list" | "ls" | "history" | "get" | "show" | "search" | "pull" | "version"
        | "template" | "lint" => (Fetch, Outward),
        _ => (OutwardKind::Other, Outward),
    };
    t.reach(
        kind,
        class,
        format!("`helm {sub}` reaches a cluster or a registry"),
    );
}

/// Why one push refspec is more than a plain branch push, if it is.
fn refspec(spec: &str) -> Option<String> {
    let plain = |r: &str| {
        !r.is_empty() && !r.contains(['*', '+', ':', '^', '~', '?', '[', '\\']) && !r.contains("..")
    };
    let (src, dst) = match spec.split_once(':') {
        Some((src, dst)) => (src, Some(dst)),
        None => (spec, None),
    };
    let src_ok =
        plain(src) && (!src.starts_with("refs/") || src.starts_with("refs/heads/")) && src != "tag";
    // A destination is the source's own branch, or any branch for `HEAD`: never another name.
    let dst_ok = dst.is_none_or(|d| {
        let heads = d.strip_prefix("refs/heads/");
        plain(d)
            && (d == src
                || heads == Some(src)
                || (src == "HEAD" && heads.is_some_and(|b| !b.is_empty())))
    });
    (!src_ok || !dst_ok).then(|| {
        format!("`{spec}` forces, deletes, renames or reaches a tag or a ref outside the branches")
    })
}

/// `gh api`: only a plain GET reads. Any other method, a field or an input body (which make it a
/// POST), or GraphQL (a mutation cannot be told from a query by its words) writes on GitHub.
fn api_writes(words: &[Option<&str>]) -> bool {
    let mut i = 0;
    while i < words.len() {
        let Some(w) = words[i] else {
            i += 1;
            continue;
        };
        // A method known only at run time is not a GET; a flag with nothing after it yet (a
        // prefix of the arguments) names none.
        let method = if matches!(w, "-X" | "--method") {
            i += 1;
            words.get(i).map(|m| m.unwrap_or("?"))
        } else {
            w.strip_prefix("--method=")
                .or_else(|| w.strip_prefix("-X").filter(|m| !m.is_empty()))
        };
        if method.is_some_and(|m| !m.eq_ignore_ascii_case("get")) {
            return true;
        }
        let field = ["-f", "-F", "--field", "--raw-field", "--input"]
            .iter()
            .any(|f| w == *f || w.starts_with(&format!("{f}=")))
            || ((w.starts_with("-f") || w.starts_with("-F")) && !w.starts_with("--"));
        if field || w == "graphql" {
            return true;
        }
        i += 1;
    }
    false
}
