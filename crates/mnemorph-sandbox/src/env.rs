//! The environment the harness gives every process it starts: its own, without the secrets.
//! The harness's environment holds the provider keys; a child's `/proc/<pid>/environ` is open
//! to every process of the user while it runs, so no child is started with them.

use std::process::Command;

/// Words that mark an environment variable as a secret, matched in its name.
const SECRET_WORDS: &[&str] = &["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "CREDENTIAL"];
/// Words that mark a secret only as a whole `_`-separated part of the name (`NPM_AUTH`, not
/// `AUTHOR`).
const SECRET_PARTS: &[&str] = &["AUTH", "PAT"];

/// The process's environment without its secrets: names carrying a secret word, values that are
/// URLs with credentials in them, and git's config given in the environment, whole
/// ([`git_env_config`]).
#[must_use]
pub fn scrubbed_env() -> Vec<(String, String)> {
    scrub(std::env::vars())
}

/// `vars` without their secrets, as [`scrubbed_env`] gives the process's.
fn scrub(vars: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    vars.into_iter()
        .filter(|(k, v)| !is_secret(k, v) && !git_env_config(k))
        .collect()
}

/// Whether `name` is part of git's config given in the environment: `GIT_CONFIG_COUNT` and its
/// `GIT_CONFIG_KEY_<n>` and `GIT_CONFIG_VALUE_<n>`. Dropped together: a key is named `KEY`, so
/// it is scrubbed, and a count left without its keys fails every git call ("missing config
/// key"); a value may carry a token (`http.extraHeader`).
fn git_env_config(name: &str) -> bool {
    name == "GIT_CONFIG_COUNT"
        || name.starts_with("GIT_CONFIG_KEY_")
        || name.starts_with("GIT_CONFIG_VALUE_")
}

/// Whether `name=value` is a secret: a secret word in its name, or a URL with credentials in
/// its value.
#[must_use]
pub fn is_secret(name: &str, value: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_WORDS.iter().any(|w| upper.contains(w))
        || upper.split('_').any(|part| SECRET_PARTS.contains(&part))
        || has_userinfo(value)
}

/// `scheme://user:pass@host` — a URL carrying credentials.
fn has_userinfo(value: &str) -> bool {
    value.split_once("://").is_some_and(|(scheme, rest)| {
        !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
            && rest
                .split(['/', '?', '#'])
                .next()
                .is_some_and(|authority| authority.contains('@'))
    })
}

/// A `git` command, not yet run, started with [`scrubbed_env`]: the harness's own git calls
/// carry no key, whatever their caller adds. Nor do they run what a repository's config names:
/// a world's `.git/config` is a body's to write, so its filesystem monitor is off (`core.fsmonitor`
/// empty — an older git reads `false` as a hook's path), its hooks point at `/dev/null`, and the
/// system config is not read (`GIT_CONFIG_NOSYSTEM`).
#[must_use]
pub fn git() -> Command {
    let mut cmd = Command::new("git");
    cmd.env_clear()
        .envs(scrubbed_env())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["-c", "core.fsmonitor=", "-c", "core.hooksPath=/dev/null"]);
    cmd
}

#[cfg(test)]
mod tests {
    use super::{git, has_userinfo, is_secret, scrub};

    /// Git's config given in the environment (`GIT_CONFIG_COUNT`, `GIT_CONFIG_KEY_<n>`,
    /// `GIT_CONFIG_VALUE_<n>`) is dropped whole: its keys are named `KEY`, and a count left without
    /// them fails every git call ("missing config key"); its values may carry a token
    /// (`http.extraHeader`).
    #[test]
    fn gits_config_from_the_environment_is_dropped_whole() {
        let path = std::env::var("PATH").unwrap_or_default();
        let given = [
            ("PATH", path.as_str()),
            ("GIT_CONFIG_COUNT", "2"),
            ("GIT_CONFIG_KEY_0", "http.extraHeader"),
            ("GIT_CONFIG_VALUE_0", "Authorization: Bearer abc"),
            ("GIT_CONFIG_KEY_1", "user.name"),
            ("GIT_CONFIG_VALUE_1", "me"),
            ("GIT_AUTHOR_NAME", "me"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()));
        let env = scrub(given);
        let names: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["PATH", "GIT_AUTHOR_NAME"]);
        let dir = std::env::temp_dir().join(format!("mnemorph-git-envcfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = std::process::Command::new("git")
            .env_clear()
            .envs(env)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .arg("-C")
            .arg(&dir)
            .args(["rev-parse", "--is-inside-work-tree"])
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!err.contains("missing config key"), "{err}");
    }

    #[test]
    fn git_starts_without_the_secrets() {
        let cmd = git();
        let set: Vec<_> = cmd.get_envs().collect();
        assert!(
            set.iter()
                .all(|(k, _)| !is_secret(&k.to_string_lossy(), ""))
        );
    }

    /// A repository's own config runs nothing through the harness's git: neither the filesystem
    /// monitor it names nor its hooks.
    #[cfg(unix)]
    #[test]
    fn a_repositorys_config_runs_nothing() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!("mnemorph-git-hostile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            git()
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .output()
                .unwrap()
        };
        assert!(run(&["init", "-q"]).status.success());
        let marker = dir.join("ran");
        let script = dir.join("hook.sh");
        std::fs::write(&script, format!("#!/bin/sh\ntouch {}\n", marker.display())).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let hooks = dir.join(".git/hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        std::fs::copy(&script, hooks.join("pre-commit")).unwrap();
        let config = dir.join(".git/config");
        let mut text = std::fs::read_to_string(&config).unwrap();
        {
            use std::fmt::Write as _;
            let _ = write!(text, "[core]\n\tfsmonitor = {}\n", script.display());
        }
        std::fs::write(&config, text).unwrap();
        std::fs::write(dir.join("a"), "a").unwrap();
        assert!(run(&["add", "a"]).status.success());
        assert!(run(&["status", "--porcelain"]).status.success());
        assert!(run(&["ls-files", "-z"]).status.success());
        let out = run(&["commit", "-q", "-m", "a"]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!marker.exists(), "a hook or the monitor ran");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn secrets_are_known_by_name_and_by_credentialed_urls() {
        for name in [
            "AWS_SECRET_ACCESS_KEY",
            "GITHUB_TOKEN",
            "DB_PASSWORD",
            "OPENROUTER_API_KEY",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "NPM_AUTH",
        ] {
            assert!(is_secret(name, "x"), "{name}");
        }
        assert!(is_secret("DATABASE_URL", "postgres://me:hunter2@db/app"));
        assert!(has_userinfo("https://token@github.com/x/y.git"));
        for (name, value) in [
            ("PATH", "/usr/bin"),
            ("HOME", "/h"),
            ("SITE", "https://example.com/a@b"),
            ("LANG", "C.UTF-8"),
            ("GIT_AUTHOR_NAME", "me"),
        ] {
            assert!(!is_secret(name, value), "{name}={value}");
        }
    }
}
