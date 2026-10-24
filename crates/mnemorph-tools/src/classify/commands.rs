//! One simple command, by name: wrappers unwrapped, then the table.

use mnemorph_races::ToolClass::{Destructive, Outward, Write};

use super::paths::Act;
use super::words::Lit;
use super::{Kind, Tally, Walker, base_name};

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

/// Classify `argv` (the command name first), raising `t`.
pub(super) fn classify(w: &mut Walker<'_>, argv: &[Lit], t: &mut Tally) -> Kind {
    let Some(name) = argv[0].text.as_deref() else {
        t.refuse("the command name is known only at run time");
        return Kind::default();
    };
    let name = base_name(name);
    let tail = &argv[1..];
    let words: Vec<Option<&str>> = tail.iter().map(|a| a.text.as_deref()).collect();
    match name {
        "sudo" | "doas" => {
            t.raise(Destructive, format!("`{name}` runs as root"));
            let rest = skip_options(
                tail,
                &["-u", "-g", "-h", "-p", "-C", "-D", "-r", "-t", "-U"],
            );
            inner(w, rest, t)
        }
        "env" => {
            let mut rest = skip_options(tail, &["-u", "-C", "-S", "--unset", "--chdir"]);
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
                None => t.refuse("`eval` runs a string known only at run time"),
            }
            Kind::default()
        }
        "source" | "." => {
            t.raise(Write, format!("`{name}` runs a script"));
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
            Some(None) => t.refuse(format!("`{name} -c` runs a string known only at run time")),
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
        t.refuse("in-place edits run blind (`perl -i`): use the edit tool");
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
            "publish" | "yank" | "owner" => t.raise(
                Destructive,
                format!("`cargo {sub}` publishes to a registry"),
            ),
            "install" | "fetch" | "update" | "search" | "login" | "logout" | "add"
            | "generate-lockfile" => {
                t.raise(Outward, format!("`cargo {sub}` reaches the registry"));
            }
            _ => t.raise(Write, "`cargo` builds and runs code"),
        },
        "npm" | "pnpm" | "yarn" => match sub {
            "publish" | "unpublish" | "deprecate" | "dist-tag" | "owner" => {
                t.raise(
                    Destructive,
                    format!("`{name} {sub}` publishes to a registry"),
                );
            }
            "install" | "i" | "add" | "update" | "up" | "upgrade" | "ci" | "dlx" | "login"
            | "logout" | "whoami" | "view" | "info" | "outdated" | "audit" => {
                t.raise(Outward, format!("`{name} {sub}` reaches the registry"));
            }
            _ => t.raise(Write, format!("`{name}` runs scripts")),
        },
        "pip" | "pip3" | "uv" | "poetry" | "pipx" => match sub {
            "upload" | "publish" => t.raise(
                Destructive,
                format!("`{name} {sub}` publishes to a registry"),
            ),
            "install" | "download" | "add" | "sync" | "lock" | "update" | "search" => {
                t.raise(Outward, format!("`{name} {sub}` reaches the registry"));
            }
            _ => t.raise(Write, format!("`{name}` runs code")),
        },
        "twine" => t.raise(Destructive, "`twine` publishes to a registry"),
        "go" => match sub {
            "get" | "install" => t.raise(Outward, format!("`go {sub}` reaches the network")),
            "mod"
                if words
                    .iter()
                    .flatten()
                    .any(|w| *w == "download" || *w == "tidy") =>
            {
                t.raise(Outward, "`go mod` reaches the network");
            }
            _ => t.raise(Write, "`go` builds and runs code"),
        },
        "docker" | "podman" => match sub {
            "push" | "rm" | "rmi" | "prune" | "kill" => {
                t.raise(Destructive, format!("`{name} {sub}` publishes or deletes"));
            }
            "system" | "volume" | "image" | "container" | "network" | "builder"
                if words.iter().flatten().any(|w| matches!(*w, "prune" | "rm")) =>
            {
                t.raise(
                    Destructive,
                    format!("`{name} {sub}` deletes outside the world"),
                );
            }
            "ps" | "images" | "inspect" | "logs" | "version" | "info" => {}
            _ => t.raise(Outward, format!("`{name}` reaches beyond the world")),
        },
        "brew" | "apt" | "apt-get" | "dnf" | "yum" | "pacman" | "port" => match sub {
            "uninstall" | "remove" | "rm" | "purge" | "autoremove" | "cleanup" | "-R" | "-Rs" => {
                t.raise(
                    Destructive,
                    format!("`{name} {sub}` deletes outside the world"),
                );
            }
            "list" | "info" | "search" | "show" | "--version" | "-Q" => {}
            _ => t.raise(Outward, format!("`{name}` installs outside the world")),
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
            _ => t.refuse("`op` reads secrets from 1Password"),
        },
        "security" => {
            if matches!(
                sub,
                "dump-keychain" | "find-generic-password" | "find-internet-password" | "export"
            ) {
                t.refuse(format!("`security {sub}` reads the keychain"));
            }
        }
        "gpg" | "gpg2" => {
            if words
                .iter()
                .flatten()
                .any(|w| w.starts_with("--export-secret") || matches!(*w, "--decrypt" | "-d"))
            {
                t.refuse("`gpg` exports or decrypts secrets");
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
                t.refuse(format!("`diskutil {sub}` rewrites a disk"));
            }
        }
        "sed" => {
            if has_short(words, 'i') || has(words, &["--in-place"]) {
                t.refuse("in-place edits run blind (`sed -i`): use the edit tool");
            }
            for a in operands(args).into_iter().skip(1) {
                w.touch(a, Act::Read, "`sed`", t);
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
        "kill" | "pkill" | "killall" => t.raise(Write, format!("`{name}` signals processes")),
        "make" | "just" | "cmake" | "ninja" | "rustc" | "gcc" | "cc" | "clang" | "tsc"
        | "vitest" | "jest" | "pytest" | "biome" | "rustfmt" | "prettier" | "eslint" => {
            t.raise(Write, format!("`{name}` builds or runs code"));
        }
        _ if NETWORK.contains(&name) => t.raise(Outward, format!("`{name}` reaches the network")),
        _ if SECRET_STORES.contains(&name) => t.refuse(format!("`{name}` reads secrets")),
        _ if MACHINE.contains(&name) || name.starts_with("mkfs") => {
            t.refuse(format!("`{name}` rewrites disks or stops the machine"));
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
    while let Some(Some(a)) = args.get(i).map(|a| a.text.as_deref()) {
        if !a.starts_with('-') {
            break;
        }
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
        t.refuse("the git subcommand is known only at run time");
        return;
    };
    let rest = &args[i + 1..];
    let words: Vec<&str> = rest.iter().filter_map(|a| a.text.as_deref()).collect();
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
            let refspec_force = words
                .iter()
                .any(|w| !w.starts_with('-') && (w.starts_with('+') || w.starts_with(':')));
            let tags = words
                .iter()
                .any(|w| w.starts_with("refs/tags/") || *w == "tag");
            if any(&[
                "--force",
                "--force-with-lease",
                "--force-if-includes",
                "--mirror",
                "--delete",
                "--tags",
                "--follow-tags",
                "--prune",
            ]) || short('f')
                || short('d')
                || refspec_force
                || tags
            {
                t.raise(
                    Destructive,
                    "a force, delete, mirror or tag push rewrites what others see",
                );
            } else {
                t.raise(Outward, "`git push` leaves the machine");
            }
        }
        "fetch" | "pull" | "clone" | "ls-remote" | "send-email" | "request-pull" | "archive"
            if sub != "archive" || any(&["--remote"]) =>
        {
            t.raise(Outward, format!("`git {sub}` reaches a remote"));
        }
        "remote" => match words.first().copied() {
            None | Some("-v" | "get-url") => {}
            Some("update" | "show" | "prune") => t.raise(Outward, "`git remote` reaches a remote"),
            Some(_) => t.raise(Write, "`git remote` changes the repository's config"),
        },
        "submodule"
            if words
                .iter()
                .any(|w| matches!(*w, "update" | "sync" | "add")) =>
        {
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
        t.refuse("`gh auth token` prints a credential");
        return;
    }
    let method_delete = ws
        .windows(2)
        .any(|p| matches!(p[0], "-X" | "--method") && p[1].eq_ignore_ascii_case("delete"))
        || ws.iter().any(|w| {
            w.strip_prefix("--method=")
                .or_else(|| w.strip_prefix("-X"))
                .is_some_and(|m| m.eq_ignore_ascii_case("delete"))
        });
    let destructive = match pair {
        (_, "delete")
        | ("release", "create" | "upload" | "edit")
        | ("repo", "archive" | "rename" | "transfer")
        | ("secret" | "variable", "set" | "remove") => true,
        ("repo", "edit") => ws.iter().any(|w| w.starts_with("--visibility")),
        ("api", _) => method_delete,
        _ => false,
    };
    if destructive {
        t.raise(
            Destructive,
            format!(
                "`gh {} {}` deletes, publishes or changes what others see",
                pair.0, pair.1
            ),
        );
    } else {
        t.raise(Outward, "`gh` reaches GitHub");
    }
}
