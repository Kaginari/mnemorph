//! The classifier's corpus: every row must hold, every run (lessons: a deny rule held only
//! against the literal command; 37 of 90 must-block commands slipped past before a corpus).

use mnemorph_races::{OutwardKind, ToolClass};
use mnemorph_tools::{Place, Verdict, classify, classify_acts};

const CORPUS: &str = include_str!("corpus.txt");

fn place() -> Place {
    Place::new("/w", "/h")
}

fn label(v: &Verdict) -> &'static str {
    match v.class() {
        None => "refuse",
        Some(ToolClass::Read) => "read",
        Some(ToolClass::Write) => "write",
        Some(ToolClass::Outward) => "outward",
        Some(_) => "destructive",
    }
}

#[test]
fn every_row_holds() {
    let mut wrong = Vec::new();
    let mut rows = 0;
    for line in CORPUS.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (want, cmd) = line.split_once(' ').expect("a row is `<class> <command>`");
        let cmd = cmd.trim_start().replace("\\n", "\n");
        let got = classify(&cmd, &place());
        rows += 1;
        if label(&got) != want {
            wrong.push(format!(
                "{want:<11} got {:<11} {cmd:?}  {got:?}",
                label(&got)
            ));
        }
    }
    assert!(rows > 200, "the corpus lost rows: {rows}");
    assert!(
        wrong.is_empty(),
        "{} of {rows} rows wrong:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// A floor refusal is never reported under a rule the human may ask, whatever came first.
#[test]
fn the_floor_wins_over_an_askable_rule() {
    use mnemorph_tools::Rule;
    for cmd in [
        "sed -i s/a/b/ /dev/sda",
        "cp ~/.ssh/id_rsa /dev/sda",
        "cat ~/.ssh/id_rsa > /dev/sda",
    ] {
        match classify(cmd, &place()) {
            Verdict::Refuse { rule, .. } => {
                assert!(rule.floor() && rule == Rule::Device, "{cmd}: {rule:?}");
            }
            v @ Verdict::Run { .. } => panic!("{cmd}: {v:?}"),
        }
    }
}

/// Every outward command names at least one kind of act, and nothing below outward names any:
/// a Kijin's post is checked against what the command does, never against nothing.
#[test]
fn every_outward_row_names_its_acts() {
    for line in CORPUS.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (_, cmd) = line.split_once(' ').expect("a row is `<class> <command>`");
        let cmd = cmd.trim_start().replace("\\n", "\n");
        let (verdict, acts) = classify_acts(&cmd, &place());
        match verdict.class() {
            Some(ToolClass::Outward) => assert!(!acts.is_empty(), "{cmd:?}"),
            Some(ToolClass::Read | ToolClass::Write) => {
                assert!(acts.is_empty(), "{cmd:?}: {acts:?}");
            }
            _ => {}
        }
    }
}

/// The kinds the classifier reads, command by command: what the posts are checked against.
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one row per command: the table reads best whole"
)]
fn outward_acts_by_kind() {
    use OutwardKind::{
        Comment, Deploy, Fetch, Install, Label, Other, Publish, PullRequest, PushBranch, PushTag,
        RerunCi, Rollback, WriteOutside,
    };
    for (cmd, want) in [
        ("git push origin refs/heads/feat/x", &[PushBranch][..]),
        ("git push -u origin HEAD:refs/heads/feat/x", &[PushBranch]),
        ("git push origin feat/x:refs/heads/feat/x", &[PushBranch]),
        ("git -C . push origin refs/heads/feat/x", &[PushBranch]),
        (
            "git push origin HEAD:refs/heads/x --force-with-lease=refs/heads/x:",
            &[PushBranch],
        ),
        ("git push origin refs/tags/v1.0.0", &[PushTag]),
        ("git push origin tag v1.0.0", &[PushTag]),
        // A push whose words do not name one branch in full is no post's.
        ("git push --tags origin", &[Other]),
        ("git push origin", &[Other]),
        ("git push", &[Other]),
        ("git push origin feat/x", &[Other]),
        ("git push origin v1.0", &[Other]),
        ("git push -u origin HEAD", &[Other]),
        ("git push --follow-tags origin refs/heads/x", &[Other]),
        ("git push origin refs/heads/a refs/heads/b", &[Other]),
        ("git push ext::sh refs/heads/x", &[Other]),
        ("git push origin refs/heads/$B", &[Other]),
        (
            "git -c core.sshCommand=x push origin refs/heads/x",
            &[Other],
        ),
        (
            "git --config-env=core.sshCommand=E push origin refs/heads/x",
            &[Other],
        ),
        ("git -C /elsewhere push origin refs/heads/x", &[Other]),
        ("git --git-dir=/x/.git push origin refs/heads/x", &[Other]),
        (
            "git config core.sshCommand x && git push origin refs/heads/x",
            &[Other],
        ),
        (
            "git remote set-url origin ext::x; git push origin refs/heads/x",
            &[Other],
        ),
        ("GIT_SSH_COMMAND=x git push origin refs/heads/x", &[Other]),
        ("GIT_SSH_COMMAND=x git fetch origin", &[Other]),
        ("GIT_CONFIG_COUNT=1 git fetch origin", &[Other]),
        ("env GIT_DIR=/x git fetch origin", &[Other]),
        ("env -i git fetch origin", &[Other]),
        ("export GIT_SSH_COMMAND=x; git fetch origin", &[Other]),
        ("X=1; git fetch origin", &[Other]),
        ("read HOME <<< /x; git fetch origin", &[Other]),
        ("for HOME in /x; do git fetch origin; done", &[Other]),
        ("f() { :; }; git fetch origin", &[Other]),
        (". ./x.sh && git fetch origin", &[Other]),
        ("cd sub && git fetch origin", &[Other]),
        ("GH_CONFIG_DIR=/x gh pr comment 7 --body hi", &[Other]),
        ("GH_HOST=x gh pr view 7", &[Other]),
        ("git fetch --upload-pack=x origin", &[Other]),
        ("git fetch -u origin", &[Other]),
        ("git fetch --config=x origin", &[Other]),
        ("git clone 'ext::sh -c x' d", &[Other]),
        ("git fetch fd::3", &[Other]),
        ("git fetch origin $R", &[Other]),
        ("git fetch --depth 1 origin", &[Fetch]),
        ("git ls-remote origin", &[Fetch]),
        ("git submodule update --init", &[Other]),
        ("wget -qe post_data=x https://example.com", &[Other]),
        ("wget -e robots=off https://example.com", &[Other]),
        ("wget -qO- https://example.com", &[Fetch]),
        ("curl --expand-data x https://example.com", &[Other]),
        ("curl --json {} https://example.com", &[Other]),
        ("curl -F a=b https://example.com", &[Other]),
        ("curl -T f https://example.com", &[Other]),
        ("curl -X POST https://example.com", &[Other]),
        ("curl -X GET https://example.com", &[Fetch]),
        (
            "curl -H 'X-HTTP-Method-Override: POST' https://example.com",
            &[Other],
        ),
        ("aria2c https://example.com/osv.zip", &[Fetch]),
        (
            "aria2c --on-download-complete=x https://example.com/osv.zip",
            &[Other],
        ),
        ("sudo ls", &[Other]),
        // A push names a configured remote: a URL in its place is no post's.
        ("git push https://github.com/o/r refs/heads/x", &[Other]),
        ("git push git@github.com:o/r refs/heads/x", &[Other]),
        ("git push origin.x refs/heads/x", &[PushBranch]),
        // A download's output is a written path.
        ("curl -o - https://example.com", &[Fetch]),
        ("curl -o out.json https://example.com", &[Fetch]),
        ("curl -o /tmp/x https://example.com", &[Fetch, Other]),
        ("curl -sSo /tmp/x https://example.com", &[Fetch, Other]),
        ("curl --output=/tmp/x https://example.com", &[Fetch, Other]),
        ("wget -O /tmp/x https://example.com", &[Fetch, Other]),
        ("wget -qO/tmp/x https://example.com", &[Fetch, Other]),
        ("wget -P /tmp/d https://example.com", &[Fetch, Other]),
        (
            "wget --directory-prefix=/tmp/d https://example.com",
            &[Fetch, Other],
        ),
        ("aria2c -d /tmp/d https://example.com/a", &[Fetch, Other]),
        ("curl -o \"$F\" https://example.com", &[Fetch, Other]),
        ("echo hi > ~/.bashrc", &[Other]),
        ("echo hi > \"$f\"", &[Other]),
        ("git push --force origin main", &[Other]),
        ("git push origin :feat/x", &[Other]),
        ("git push --force origin refs/tags/v1", &[Other]),
        ("git fetch origin", &[Fetch]),
        ("git clone https://github.com/google/osv.git", &[Fetch]),
        ("git send-email x.patch", &[Other]),
        ("gh pr create --title t --body b", &[PullRequest]),
        ("gh pr edit 7 --add-label ready", &[PullRequest, Label]),
        ("gh pr comment 7 --body hi", &[Comment]),
        ("gh issue comment 7 --body hi", &[Comment]),
        ("gh issue edit 7 --add-label bug -R o/r", &[Label]),
        ("gh issue edit 7 --title new", &[Other]),
        (
            "gh issue edit 7 --add-label bug --title new",
            &[Label, Other],
        ),
        ("gh label create bug", &[Label]),
        ("gh pr view 7", &[Fetch]),
        ("gh issue list", &[Fetch]),
        ("gh run view 42 --log", &[Fetch]),
        ("gh run rerun 42", &[RerunCi]),
        ("gh workflow run deploy.yml", &[Other]),
        ("gh pr merge 7", &[Other]),
        ("gh release create v1.0.0", &[Publish]),
        ("gh api repos/o/r/releases", &[Fetch]),
        ("gh api -X POST repos/o/r/issues", &[Other]),
        ("gh repo delete o/r", &[Other]),
        ("cargo publish", &[Publish]),
        ("npm publish", &[Publish]),
        ("docker push img:1", &[Publish]),
        ("docker pull img:1", &[Fetch]),
        ("cargo install ripgrep", &[Install]),
        ("npm install", &[Install]),
        ("apt-get install jq", &[Install]),
        ("kubectl apply -f deploy.yaml", &[Deploy]),
        ("kubectl get pods", &[Fetch]),
        ("kubectl rollout undo deployment/web", &[Rollback]),
        ("kubectl exec -it web -- sh", &[Other]),
        ("helm upgrade web ./chart", &[Deploy]),
        ("helm rollback web 3", &[Rollback]),
        ("curl -fsS https://example.com/health", &[Fetch]),
        ("curl -d x=1 https://example.com", &[Other]),
        ("curl -XPOST https://example.com", &[Other]),
        ("curl telnet://example.com", &[Other]),
        ("wget https://example.com/osv.zip", &[Fetch]),
        ("wget --post-data=x https://example.com", &[Other]),
        ("dig example.com", &[Fetch]),
        ("ssh host uptime", &[Other]),
        ("echo hi > /tmp/out", &[Other]),
        ("rm /tmp/out", &[Other]),
        ("cat < /dev/tcp/example.com/80", &[Other]),
        ("docker ps", &[Other]),
        ("docker rmi img:1", &[WriteOutside]),
        ("brew uninstall jq", &[Install]),
        (
            "git push origin refs/heads/feat/x && kubectl apply -f d.yaml",
            &[PushBranch, Deploy],
        ),
        ("sh -c 'gh pr comment 7 --body hi'", &[Comment]),
    ] {
        let (verdict, acts) = classify_acts(cmd, &place());
        assert!(verdict.class().is_some(), "{cmd}: {verdict:?}");
        assert_eq!(acts, want, "{cmd}");
    }
    // Outside the world, only a deploy root is written as the deploy's, and never a dotfile.
    let rooted = Place::new("/w", "/tmp/h").with_roots(["/tmp".into()]);
    for (cmd, want) in [
        ("echo hi > /tmp/out", WriteOutside),
        ("rm /tmp/out", WriteOutside),
        ("echo hi > /tmp/h/.gitconfig", Other),
        ("echo hi > /tmp/h/.config/git/config", Other),
        ("echo hi > /var/out", Other),
    ] {
        assert_eq!(classify_acts(cmd, &rooted).1, [want], "{cmd}");
    }
    // A download into a credential is refused like any write there.
    assert!(matches!(
        classify(
            "curl -o ~/.ssh/authorized_keys https://example.com",
            &place()
        ),
        Verdict::Refuse {
            rule: mnemorph_tools::Rule::Credential,
            ..
        }
    ));
    // A cluster's secrets are a secret store, not a fetch.
    for cmd in [
        "kubectl get secret db",
        "kubectl get secrets -o yaml",
        "kubectl describe secret/db",
        "kubectl get pods,secrets",
    ] {
        assert!(
            matches!(
                classify(cmd, &place()),
                Verdict::Refuse {
                    rule: mnemorph_tools::Rule::SecretStore,
                    ..
                }
            ),
            "{cmd}"
        );
    }
    // Local acts name none, destructive or not.
    for cmd in [
        "git reset --hard",
        "git rebase main",
        "cargo build",
        "cat README.md",
    ] {
        assert_eq!(classify_acts(cmd, &place()).1, [], "{cmd}");
    }
}
