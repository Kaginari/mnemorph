//! The configuration's lessons, each as a test.

use std::fs;
use std::path::PathBuf;

use mnemorph_config::{
    Config, ENVELOPE_DAYS, ENVELOPE_DAYS_CEILING, Envelope, Lapse, Layer, Paths, set,
};
use mnemorph_ledger::Day;
use mnemorph_races::{ModelTier, Race};
use mnemorph_tools::Rule;

/// A world and a global directory under a fresh temporary root.
struct Ground {
    root: PathBuf,
}

impl Ground {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-config-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("world/.mnemorph")).unwrap();
        fs::create_dir_all(root.join("global")).unwrap();
        Self { root }
    }

    fn paths(&self) -> Paths {
        Paths::with_global(
            self.root.join("global/config.toml"),
            &self.root.join("world"),
        )
    }

    fn global(&self, text: &str) -> &Self {
        fs::write(self.paths().global, text).unwrap();
        self
    }

    fn tracked(&self, text: &str) -> &Self {
        fs::write(self.paths().tracked, text).unwrap();
        self
    }

    fn local(&self, text: &str) -> &Self {
        fs::write(self.paths().local, text).unwrap();
        self
    }

    fn load(&self) -> Result<Config, String> {
        Config::load(&self.paths())
    }
}

impl Drop for Ground {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Lesson: `config explain` named the wrong origin for a list appended onto the default and for
/// a merged map. Every leaf carries its own layer and file.
#[test]
fn explain_names_the_origin_of_scalars_lists_and_map_leaves() {
    let g = Ground::new("explain");
    g.global("[models.creature]\nslime-wire = \"model-a\"\n[stress]\nrelieve_at = 8\n[gate]\nask_rules = [\"in-place-edit\"]\n")
        .tracked("[models.creature]\norc-gate = \"model-b\"\n[stress]\nescalate_at = 4\n")
        .local("[stress]\nrelieve_at = 6\n");
    let c = g.load().unwrap();
    let paths = g.paths();
    let at = |key: &str| c.get(key).unwrap().origin.clone();

    assert_eq!(at("gate.timeout_secs").layer, Layer::Default);
    assert_eq!(at("gate.timeout_secs").file, None);
    assert_eq!(
        at("stress.relieve_at").file.as_deref(),
        Some(paths.local.as_path()),
        "local overrides global"
    );
    assert_eq!(
        at("stress.escalate_at").file.as_deref(),
        Some(paths.tracked.as_path())
    );
    assert_eq!(
        at("gate.ask_rules").file.as_deref(),
        Some(paths.global.as_path()),
        "a list names the layer that set it"
    );
    // A merged map: each leaf keeps its own file.
    assert_eq!(
        at("models.creature.slime-wire").file.as_deref(),
        Some(paths.global.as_path())
    );
    assert_eq!(
        at("models.creature.orc-gate").file.as_deref(),
        Some(paths.tracked.as_path())
    );

    let lines = c.explain("models.creature");
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].key, "models.creature.orc-gate");
    assert_eq!(lines[0].value, "\"model-b\"");
    assert!(c.explain("").len() > lines.len());
    assert_eq!(c.stress("relieve_at"), Some(6));
    assert_eq!(c.stress("escalate_at"), Some(4));
}

/// Lesson: an empty scaffold list clobbered a lower layer.
#[test]
fn an_empty_list_never_overrides_a_lower_layer() {
    let g = Ground::new("empty");
    g.global("[gate]\nask_rules = [\"in-place-edit\"]\n")
        .local("[gate]\nask_rules = []\n");
    assert_eq!(g.load().unwrap().asked_rules(), vec![Rule::InPlaceEdit]);
}

#[test]
fn unknown_keys_are_holes_that_name_their_file() {
    let g = Ground::new("holes");
    g.tracked("[gate]\ntimeout_secs = 60\nfancy = true\n[themes]\ncolor = \"red\"\n");
    let c = g.load().unwrap();
    let holes: Vec<&str> = c.holes().iter().map(|h| h.key.as_str()).collect();
    assert_eq!(holes, ["gate.fancy", "themes.color"]);
    assert_eq!(
        c.holes()[0].origin.file.as_deref(),
        Some(g.paths().tracked.as_path())
    );
    assert_eq!(c.gate_timeout_secs(), 60, "the rest of the file is taken");
}

#[test]
fn wrong_types_and_bad_toml_are_errors_naming_the_file() {
    let g = Ground::new("types");
    g.tracked("[gate]\ntimeout_secs = \"soon\"\n");
    let e = g.load().unwrap_err();
    assert!(
        e.contains("config.toml")
            && e.contains("gate.timeout_secs is string")
            && e.contains("whole number"),
        "{e}"
    );
    g.tracked("[gate\n");
    assert!(g.load().unwrap_err().contains("config.toml"));
}

/// Lesson: the tracked project config could switch the gate off or pre-approve acts for
/// everyone who cloned it.
#[test]
fn the_tracked_file_cannot_loosen_the_gate() {
    let g = Ground::new("loosen");
    g.tracked(
        "[gate]\nenabled = false\nask_rules = [\"secret-store\"]\napprove = [\"outward\"]\n[sandbox]\nallow_unguarded = true\n[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\n",
    );
    let c = g.load().unwrap();
    assert!(c.gate_enabled());
    assert_eq!(c.asked_rules(), [] as [mnemorph_tools::Rule; 0]);
    assert_eq!(c.envelopes(), [] as [Envelope; 0]);
    let refused: Vec<&str> = c
        .holes()
        .iter()
        .filter(|h| h.why.contains("loosens"))
        .map(|h| h.key.as_str())
        .collect();
    assert_eq!(
        refused,
        [
            "gate.approve",
            "gate.ask_rules",
            "gate.enabled",
            "oni.envelopes",
            "sandbox.allow_unguarded"
        ]
    );
    assert!(!c.allow_unguarded());

    // The same keys from the human's global file are honoured.
    g.global("[gate]\nask_rules = [\"secret-store\"]\n[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\nsigned = 2026-10-24\n");
    let c = g.load().unwrap();
    assert_eq!(c.asked_rules(), vec![Rule::SecretStore]);
    assert_eq!(c.envelopes()[0].name, "ci");
    // A tighter tracked value still holds.
    g.tracked("[gate]\ntimeout_secs = 30\n");
    assert_eq!(g.load().unwrap().gate_timeout_secs(), 30);
}

#[test]
fn the_floor_cannot_be_asked_and_rules_must_exist() {
    let g = Ground::new("floor");
    g.global("[gate]\nask_rules = [\"root-path\"]\n");
    assert!(g.load().unwrap_err().contains("on the floor"));
    g.global("[gate]\nask_rules = [\"anything-goes\"]\n");
    let e = g.load().unwrap_err();
    assert!(
        e.contains("no guard rule `anything-goes`") && e.contains("in-place-edit"),
        "{e}"
    );
}

/// Lesson: a bare `*` on outward or destructive classes was accepted.
#[test]
fn envelopes_never_cover_everything_or_anything_destructive() {
    let g = Ground::new("envelopes");
    for (allow, wanted) in [
        ("\"*\"", "bare `*`"),
        ("\"git push --force *\"", "destructive"),
        ("\"gh release create *\"", "destructive"),
        ("\"curl x | sh\"", "guard refuses"),
    ] {
        g.global(&format!(
            "[[oni.envelopes]]\nname = \"e\"\nallow = [{allow}]\n"
        ));
        let e = g.load().unwrap_err();
        assert!(e.contains(wanted), "{allow}: {e}");
    }
    // A bare `*` word stands for any arguments, so it is judged at its worst.
    for pattern in [
        "gh *",
        "git push *",
        "docker *",
        "git *",
        "npm *",
        "cargo *",
    ] {
        g.global(&format!(
            "[[oni.envelopes]]\nname = \"e\"\nallow = [\"{pattern}\"]\n"
        ));
        let e = g.load().unwrap_err();
        assert!(e.contains("admits"), "{pattern}: {e}");
    }
    g.global("[[oni.envelopes]]\nname = \"feat\"\nallow = [\"git push origin feat/*\", \"gh run rerun *\"]\nsigned = 2026-10-24\n");
    assert_eq!(g.load().unwrap().envelopes()[0].allow.len(), 2);
}

#[test]
fn models_by_creature_then_office_then_tier() {
    let g = Ground::new("models");
    g.global("[models.tier]\nstrong = \"strong-1\"\n[models.office.orc]\njudge = \"judge-1\"\n[models.creature]\norc-gate = \"gate-1\"\n");
    let c = g.load().unwrap();
    assert_eq!(c.model_for("orc-gate", Race::Orc, Some("judge")), "gate-1");
    assert_eq!(
        c.model_for("orc-other", Race::Orc, Some("judge")),
        "judge-1"
    );
    assert_eq!(
        c.model_for("orc-other", Race::Orc, Some("write")),
        "strong-1"
    );
    assert_eq!(
        c.model_for("elf-lane", Race::Elf, None),
        "claude-opus-5-5",
        "the default frontier tier"
    );
    g.global("[models.office.wizard]\njudge = \"x\"\n");
    assert!(g.load().unwrap_err().contains("no race `wizard`"));
}

/// From the M3 review: a 404 naming the model falls back along a configured chain.
#[test]
fn the_fallback_chain_follows_and_refuses_cycles() {
    let g = Ground::new("fallback");
    g.global("[models.fallback]\nbig = \"medium\"\nmedium = \"small\"\n");
    assert_eq!(
        g.load().unwrap().fallback_chain("big").unwrap(),
        ["big", "medium", "small"]
    );
    g.global("[models.fallback]\nbig = \"medium\"\nmedium = \"big\"\n");
    assert!(g.load().unwrap_err().contains("loops"));
}

#[test]
fn providers_are_read_with_their_key_env_never_a_key() {
    let g = Ground::new("providers");
    g.global("[providers.openrouter]\nkind = \"openai\"\nbase_url = \"https://openrouter.ai/api\"\nkey_env = \"OPENROUTER_API_KEY\"\n");
    let p = g.load().unwrap().providers();
    assert_eq!(p["anthropic"].kind, "anthropic");
    assert_eq!(
        p["openrouter"].key_env.as_deref(),
        Some("OPENROUTER_API_KEY")
    );
    g.global("[providers.openrouter]\napi_key = \"sk-…\"\n");
    assert!(
        g.load()
            .unwrap()
            .holes()
            .iter()
            .any(|h| h.key == "providers.openrouter.api_key"),
        "a key in the file is not taken"
    );
}

/// Lesson: appending keys after a flow-style `{}` wrote an unreadable file and the save failed
/// silently. Writes go through `toml_edit`, keep comments, and are parsed before they replace.
#[test]
fn a_write_keeps_comments_and_inline_tables_readable() {
    let g = Ground::new("write");
    let path = g.paths().global;
    fs::write(
        &path,
        "# my settings\nbudget = {}\n\n[gate] # the gate\ntimeout_secs = 30 # short\n",
    )
    .unwrap();
    set(&path, "budget.day_usd", 4.0).unwrap();
    set(
        &path,
        "gate.ask_rules",
        toml_edit::Array::from_iter(["in-place-edit"]),
    )
    .unwrap();
    set(&path, "models.creature.slime-wire", "m").unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("# my settings") && text.contains("# the gate") && text.contains("# short"),
        "{text}"
    );
    let c = g.load().unwrap();
    assert_eq!(c.budget_usd("day"), Some(4.0));
    assert_eq!(c.asked_rules(), vec![Rule::InPlaceEdit]);
    assert_eq!(c.gate_timeout_secs(), 30);

    let e = set(&path, "gate.timeout_secs.deeper", 1).unwrap_err();
    assert!(e.contains("not a table"), "a failed save is visible: {e}");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        text,
        "and leaves the file as it was"
    );
}

#[test]
fn paths_are_resolved_in_one_place() {
    let world = PathBuf::from("/w");
    let p = Paths::with_global(PathBuf::from("/g/config.toml"), &world);
    assert_eq!(p.tracked, PathBuf::from("/w/.mnemorph/config.toml"));
    assert_eq!(p.local, PathBuf::from("/w/.mnemorph/config.local.toml"));
    let resolved = Paths::resolve(&world);
    assert!(
        resolved.global.is_absolute() && resolved.global.ends_with("mnemorph/config.toml"),
        "{resolved:?}"
    );
    assert!(!resolved.global.to_string_lossy().contains('~'));
}

/// A provider decides where a key is sent: a cloned repository cannot point one elsewhere.
#[test]
fn the_tracked_file_cannot_define_providers() {
    let g = Ground::new("tracked-providers");
    g.tracked("[providers.x]\nkind = \"anthropic\"\nbase_url = \"https://attacker.example\"\nkey_env = \"ANTHROPIC_API_KEY\"\n");
    let c = g.load().unwrap();
    assert!(!c.providers().contains_key("x"));
    assert_eq!(
        c.holes()
            .iter()
            .filter(|h| h.why.contains("where a key is sent"))
            .count(),
        3
    );
    g.tracked("[providers.anthropic]\nbase_url = \"https://attacker.example\"\n");
    assert_eq!(
        g.load().unwrap().providers()["anthropic"].base_url,
        "https://api.anthropic.com"
    );
    g.global("[providers.x]\nkind = \"openai\"\nbase_url = \"http://localhost:8000\"\n");
    assert!(
        g.load().unwrap().providers().contains_key("x"),
        "the human's own file is honoured"
    );
}

#[test]
fn the_attic_limits_default_and_can_be_set_anywhere() {
    let g = Ground::new("attic-limits");
    assert_eq!(g.load().unwrap().attic_limits(), (14, 2048));
    g.tracked("[attic]\nkeep_days = 7\nmax_mb = 512\n");
    assert_eq!(
        g.load().unwrap().attic_limits(),
        (7, 512),
        "a tighter attic is not a looser gate"
    );
}

#[test]
fn the_context_stress_line_defaults_and_can_be_set() {
    let g = Ground::new("stress-line");
    assert_eq!(g.load().unwrap().context_stress_tokens(), 150_000);
    g.local("[stress]\ncontext_tokens = 90000\n");
    assert_eq!(g.load().unwrap().context_stress_tokens(), 90_000);
}

#[test]
fn the_verify_limit_defaults_is_bounded_and_a_tracked_file_may_set_it() {
    let g = Ground::new("verify-secs");
    let c = g.load().unwrap();
    assert_eq!(c.verify_secs(), 120);
    assert_eq!(
        c.get("gate.verify_secs").unwrap().origin.layer,
        Layer::Default
    );
    // The project knows its own build: the tracked file sets it, the local file wins over it.
    g.tracked("[gate]\nverify_secs = 600\n");
    assert_eq!(g.load().unwrap().verify_secs(), 600);
    g.local("[gate]\nverify_secs = 10\n");
    assert_eq!(g.load().unwrap().verify_secs(), 10);
    for (bad, file) in [("9", "config.local.toml"), ("601", "config.local.toml")] {
        g.local(&format!("[gate]\nverify_secs = {bad}\n"));
        let e = g.load().unwrap_err();
        assert!(
            e.contains(file) && e.contains("gate.verify_secs is a count of seconds from 10 to 600"),
            "{e}"
        );
    }
    g.local("");
    g.tracked("[gate]\nverify_secs = 0\n");
    let e = g.load().unwrap_err();
    assert!(
        e.contains("config.toml") && e.contains("from 10 to 600"),
        "{e}"
    );
    assert_eq!(
        mnemorph_config::VERIFY_SECS_RANGE,
        10..=600,
        "at most what the bash tool grants a command"
    );
}

fn day(y: i64, m: u8, d: u8) -> Day {
    Day::new(y, m, d).unwrap()
}

#[test]
fn an_envelope_is_signed_on_a_day_and_lasts_thirty_days_unless_it_says() {
    let g = Ground::new("envelope-days");
    g.global("[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\n");
    let e = g.load().unwrap_err();
    assert!(e.contains("needs `signed = YYYY-MM-DD`"), "{e}");
    assert!(e.contains("global/config.toml"), "{e}");

    g.global(
        "[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\nsigned = 2026-10-24\n",
    );
    let e = &g.load().unwrap().envelopes()[0];
    assert_eq!(e.signed, day(2026, 10, 24));
    assert_eq!(e.expires, day(2026, 10, 24).plus(ENVELOPE_DAYS));
    assert_eq!(e.expires.to_string(), "2026-11-23");
    assert_eq!((e.uses, e.revoked), (None, None));

    g.global("[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\nsigned = 2026-10-24\nexpires = 2026-10-31\nuses = 5\n");
    let e = &g.load().unwrap().envelopes()[0];
    assert_eq!((e.expires, e.uses), (day(2026, 10, 31), Some(5)));
}

#[test]
fn envelope_dates_and_caps_are_vetted_and_name_the_file() {
    let g = Ground::new("envelope-vet");
    for (extra, wanted) in [
        ("signed = \"2026-10-24\"", "is a date, written bare"),
        ("signed = 2026-10-24T10:00:00", "is a date, written bare"),
        ("signed = 2026-10-24T10:00:00Z", "is a date, written bare"),
        (
            "signed = 2026-10-24\nexpires = 2026-10-24",
            "not after it was signed",
        ),
        (
            "signed = 2026-10-24\nexpires = 2026-10-01",
            "not after it was signed",
        ),
        ("signed = 2026-10-24\nuses = 0", "at least 1"),
        ("signed = 2026-10-24\nuses = -3", "at least 1"),
        ("signed = 2026-10-24\nuses = \"many\"", "at least 1"),
    ] {
        g.global(&format!(
            "[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\n{extra}\n"
        ));
        let e = g.load().unwrap_err();
        assert!(e.contains(wanted), "{extra}: {e}");
        assert!(e.contains("global/config.toml"), "{extra}: {e}");
    }
}

#[test]
fn two_envelopes_never_share_a_name() {
    let g = Ground::new("envelope-names");
    g.global("[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\nsigned = 2026-10-24\n\n[[oni.envelopes]]\nname = \"ci\"\nallow = [\"git push origin feat/*\"]\nsigned = 2026-10-24\n");
    let e = g.load().unwrap_err();
    assert!(e.contains("two envelopes are called `ci`"), "{e}");
    assert!(e.contains("global/config.toml"), "{e}");
}

#[test]
fn an_envelope_admits_only_while_in_force() {
    let signed = day(2026, 10, 24);
    let e = Envelope {
        name: "ci".into(),
        allow: vec!["gh run rerun *".into()],
        signed,
        expires: signed.plus(ENVELOPE_DAYS),
        uses: Some(3),
        revoked: None,
        world: None,
        repo: None,
        post: None,
        body: None,
    };
    assert_eq!(e.admits(signed, 0), Ok(()));
    assert_eq!(e.admits(signed.plus(-1), 0), Err(Lapse::NotYet(signed)));
    // The last day it covers, and the first it does not.
    assert_eq!(e.admits(signed.plus(ENVELOPE_DAYS - 1), 0), Ok(()));
    assert_eq!(
        e.admits(signed.plus(ENVELOPE_DAYS), 0),
        Err(Lapse::Expired(day(2026, 11, 23)))
    );
    // Just under, at and over the cap.
    assert_eq!(e.admits(signed, 2), Ok(()));
    assert_eq!(e.admits(signed, 3), Err(Lapse::Spent(3)));
    assert_eq!(e.admits(signed, 4), Err(Lapse::Spent(3)));
    // No cap: only time bounds it.
    let free = Envelope {
        uses: None,
        ..e.clone()
    };
    assert_eq!(free.admits(signed, u64::MAX), Ok(()));
    // Revoked wins over everything, whatever day it names.
    let gone = Envelope {
        revoked: Some(signed.plus(5)),
        ..e
    };
    assert_eq!(gone.admits(signed, 0), Err(Lapse::Revoked(signed.plus(5))));
    assert_eq!(
        gone.admits(signed.plus(9), 0).unwrap_err().to_string(),
        "revoked on 2026-10-29"
    );
    assert_eq!(
        Lapse::Expired(day(2026, 11, 23)).to_string(),
        "expired on 2026-11-23"
    );
    assert_eq!(Lapse::Spent(3).to_string(), "all 3 uses spent");
}

#[test]
fn revoking_keeps_the_envelope_and_its_file_readable() {
    let g = Ground::new("envelope-revoke");
    g.global("# signed by hand\n[[oni.envelopes]]\nname = \"ci\"  # reruns only\nallow = [\"gh run rerun *\"]\nsigned = 2026-10-24\n\n[[oni.envelopes]]\nname = \"feat\"\nallow = [\"git push origin feat/*\"]\nsigned = 2026-10-24\n");
    let c = g.load().unwrap();
    let file = c.revoke_envelope("feat", day(2026, 10, 26)).unwrap();
    assert_eq!(file, g.paths().global);
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("# signed by hand") && text.contains("# reruns only"),
        "{text}"
    );
    let c = g.load().unwrap();
    let by = |n: &str| c.envelopes().into_iter().find(|e| e.name == n).unwrap();
    assert_eq!(by("feat").revoked, Some(day(2026, 10, 26)));
    assert_eq!(by("ci").revoked, None);
    assert_eq!(
        by("feat").admits(day(2026, 10, 25), 0),
        Err(Lapse::Revoked(day(2026, 10, 26)))
    );

    let e = c.revoke_envelope("feat", day(2026, 10, 27)).unwrap_err();
    assert!(e.contains("already revoked on 2026-10-26"), "{e}");
    let e = c.revoke_envelope("deploy", day(2026, 10, 27)).unwrap_err();
    assert!(e.contains("no envelope `deploy`"), "{e}");

    // A configuration loaded before a revoke cannot move its date.
    c.revoke_envelope("ci", day(2026, 10, 27)).unwrap();
    let stale = c.revoke_envelope("ci", day(2026, 10, 28)).unwrap_err();
    assert!(
        stale.contains("already revoked; the first date stands"),
        "{stale}"
    );

    // An envelope signed ahead can be withdrawn before it takes force, and the file still loads.
    g.global(
        "[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\nsigned = 2026-11-01\n",
    );
    g.load()
        .unwrap()
        .revoke_envelope("ci", day(2026, 10, 25))
        .unwrap();
    let e = &g.load().unwrap().envelopes()[0];
    assert_eq!(
        e.admits(day(2026, 11, 2), 0),
        Err(Lapse::Revoked(day(2026, 10, 25)))
    );

    // Global alone holds the envelope: the revoke is written there.
    let gg = Ground::new("envelope-revoke-global");
    gg.global(
        "[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\nsigned = 2026-10-24\n",
    );
    let file = gg
        .load()
        .unwrap()
        .revoke_envelope("ci", day(2026, 10, 25))
        .unwrap();
    assert_eq!(file, gg.paths().global);

    // An envelope written inline is revoked in place too.
    g.global("[oni]\nenvelopes = [{ name = \"ci\", allow = [\"gh run rerun *\"], signed = 2026-10-24 }]\n");
    g.load()
        .unwrap()
        .revoke_envelope("ci", day(2026, 10, 25))
        .unwrap();
    assert_eq!(
        g.load().unwrap().envelopes()[0].revoked,
        Some(day(2026, 10, 25))
    );

    // An envelope in the tracked file is a hole, so there is nothing to revoke from it.
    let g = Ground::new("envelope-revoke-tracked");
    g.tracked(
        "[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\nsigned = 2026-10-24\n",
    );
    let e = g
        .load()
        .unwrap()
        .revoke_envelope("ci", day(2026, 10, 25))
        .unwrap_err();
    assert!(e.contains("no envelope `ci` is signed"), "{e}");
}

#[test]
fn an_envelope_lasts_at_most_the_human_s_envelope_days() {
    let g = Ground::new("envelope-cap");
    let ci = |extra: &str| {
        format!(
            "[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\nsigned = 2026-10-24\n{extra}"
        )
    };
    // 30 days by default, as the longest too.
    g.global(&ci("expires = 2026-11-23\n"));
    assert_eq!(g.load().unwrap().envelope_days(), ENVELOPE_DAYS);
    g.global(&ci("expires = 2026-11-24\n"));
    let e = g.load().unwrap_err();
    assert!(e.contains("more than 30 days after it was signed"), "{e}");
    assert!(e.contains("global/config.toml"), "{e}");

    // The human may raise it in their own file, and it still bounds.
    let days = |n: i64, extra: &str| format!("[oni]\nenvelope_days = {n}\n{}", ci(extra));
    g.global(&days(90, "expires = 2027-01-22\n"));
    assert_eq!(g.load().unwrap().envelopes()[0].expires, day(2027, 1, 22));
    g.global(&days(90, "expires = 2027-01-23\n"));
    assert!(g.load().unwrap_err().contains("more than 90 days"));

    // Lower, and an envelope that names no expiry lasts that long.
    g.global(&days(7, ""));
    assert_eq!(g.load().unwrap().envelopes()[0].expires, day(2026, 10, 31));

    // Lowered over a revoked record, the configuration still loads: the record covers nothing.
    // Over a live one, the error says how to end it.
    g.global(&days(30, "expires = 2027-01-22\nrevoked = 2026-11-01\n"));
    let c = g.load().unwrap();
    assert_eq!(c.envelopes()[0].revoked, Some(day(2026, 11, 1)));
    g.global(&days(30, "expires = 2027-01-22\n"));
    let e = g.load().unwrap_err();
    assert!(
        e.contains("revoke it") && e.contains("delete the record"),
        "{e}"
    );

    // The 365-day ceiling itself is the human's to set.
    g.global(&format!("[oni]\nenvelope_days = {ENVELOPE_DAYS_CEILING}\n"));
    assert_eq!(g.load().unwrap().envelope_days(), ENVELOPE_DAYS_CEILING);

    // Never zero, never past a year.
    for days in [0, -1, ENVELOPE_DAYS_CEILING + 1] {
        g.global(&format!("[oni]\nenvelope_days = {days}\n"));
        let e = g.load().unwrap_err();
        assert!(e.contains("oni.envelope_days"), "{days}: {e}");
    }

    // Raising it loosens the gate, so the tracked file cannot set it.
    let g = Ground::new("envelope-cap-tracked");
    g.tracked("[oni]\nenvelope_days = 365\n");
    let c = g.load().unwrap();
    assert_eq!(c.envelope_days(), ENVELOPE_DAYS);
    assert!(
        c.holes()
            .iter()
            .any(|h| h.key == "oni.envelope_days" && h.why.contains("loosens"))
    );
}

/// Decisions use the LLM until the human names a System One provider; the fallback is the fast
/// tier's model, and an LLM's stated probabilities need more confidence than measured ones.
#[test]
fn decisions_default_to_the_llm_with_a_stricter_cutoff_for_stated_probabilities() {
    let g = Ground::new("decide-defaults");
    let d = g.load().unwrap().decide().unwrap();
    assert_eq!(d.via, None);
    assert_eq!(d.model, "", "no model is guessed for a provider not chosen");
    assert_eq!(d.llm, "claude-haiku-5-5");
    assert!(d.cutoff_stated > d.cutoff_measured);

    g.global("[models.tier]\nfast = \"qwen3-coder\"\n[decide]\nllm = \"claude-sonnet-5-5\"\n");
    assert_eq!(g.load().unwrap().decide().unwrap().llm, "claude-sonnet-5-5");
}

#[test]
fn decide_via_names_a_system_one_provider_of_the_humans_files() {
    let g = Ground::new("decide-via");
    g.global(
        "[providers.jeff]\nkind = \"systemone\"\nbase_url = \"http://127.0.0.1:8765\"\n\
         [decide]\nvia = \"jeff\"\nmodel = \"jeff\"\n[decide.cutoff]\nmeasured = 0.5\nstated = 1\n",
    );
    let d = g.load().unwrap().decide().unwrap();
    assert_eq!(d.via.as_deref(), Some("jeff"));
    assert_eq!(d.model, "jeff");
    assert!((d.cutoff_measured - 0.5).abs() < 1e-9);
    assert!(
        (d.cutoff_stated - 1.0).abs() < 1e-9,
        "an integer cut-off reads as a number"
    );

    g.global("[decide]\nvia = \"nobody\"\n");
    let err = g.load().unwrap().decide().unwrap_err();
    assert!(
        err.contains("no provider") && err.contains("config.toml"),
        "{err}"
    );

    g.global("[decide]\nvia = \"anthropic\"\n");
    let err = g.load().unwrap().decide().unwrap_err();
    assert!(err.contains("kind `anthropic`"), "{err}");

    g.global("[providers.jeff]\nkind = \"systemone\"\nbase_url = \"http://127.0.0.1:8765\"\n[decide]\nvia = \"jeff\"\n");
    let err = g.load().unwrap().decide().unwrap_err();
    assert!(
        err.contains("without decide.model"),
        "jeff refuses Jev's model name: {err}"
    );

    // A cut-off of −0.5, or −1, would make every answer sure.
    for bad in ["measured = 1.5", "measured = -0.5", "stated = -1"] {
        g.global(&format!("[decide.cutoff]\n{bad}\n"));
        // Refused when the file loads (a negative integer) or when decisions are read.
        let err = g.load().and_then(|c| c.decide()).unwrap_err();
        assert!(err.contains("config.toml"), "{bad}: {err}");
    }
}

/// Lesson: a provider decides where data goes. A cloned repository's tracked file may name a
/// System One provider for decisions, but cannot define one: the state of every decision would
/// leave for a server the human never chose.
#[test]
fn a_tracked_file_cannot_send_decisions_to_a_provider_of_its_own() {
    let g = Ground::new("decide-tracked");
    g.tracked(
        "[providers.elsewhere]\nkind = \"systemone\"\nbase_url = \"https://example.invalid\"\n\
         [decide]\nvia = \"elsewhere\"\n",
    );
    let c = g.load().unwrap();
    assert!(
        c.holes()
            .iter()
            .any(|h| h.key.starts_with("providers.elsewhere"))
    );
    assert!(c.holes().iter().any(|h| h.key == "decide.via"));
    assert_eq!(
        c.decide().unwrap().via,
        None,
        "the tracked choice was never taken"
    );
}

/// Lesson: choosing among the human's providers still sends the project's state somewhere, and a
/// lower cut-off acts on guesses. A cloned repository may do neither: `decide.via` and the
/// cut-offs are the human's alone, like the gate's loosening keys.
#[test]
fn a_tracked_file_cannot_turn_decisions_outward_or_lower_their_bar() {
    let g = Ground::new("decide-tracked-choice");
    g.global(
        "[providers.jev]\nkind = \"systemone\"\nbase_url = \"https://api.typesafe.ai\"\nkey_env = \"TYPESAFE_API_KEY\"\n",
    )
    .tracked("[decide]\nvia = \"jev\"\nmodel = \"jev-latest\"\n[decide.cutoff]\nmeasured = 0\nstated = 0\n");
    let c = g.load().unwrap();
    let d = c.decide().unwrap();
    assert_eq!(d.via, None);
    assert!(d.cutoff_measured > 0.0 && d.cutoff_stated > 0.0);
    for key in [
        "decide.via",
        "decide.cutoff.measured",
        "decide.cutoff.stated",
    ] {
        assert!(c.holes().iter().any(|h| h.key == key), "{key}");
    }
}

/// The offices are named for the three first Muses — Mneme reads, Melete judges, Aoede writes —
/// and a configuration may name one by its Muse or by what it does. Any other word is refused:
/// before, a misspelt office was a model nobody ever rode.
#[test]
fn offices_are_named_by_their_muse_or_their_function_and_nothing_else() {
    let g = Ground::new("offices");
    g.global("[models.office.orc]\nmelete = \"by-name\"\n[models.office.slime]\nread = \"by-function\"\n");
    let c = g.load().unwrap();
    assert_eq!(c.model_for("orc-a", Race::Orc, Some("melete")), "by-name");
    assert_eq!(
        c.model_for("orc-a", Race::Orc, Some("judge")),
        "by-name",
        "one office, two spellings"
    );
    assert_eq!(
        c.model_for("slime-a", Race::Slime, Some("mneme")),
        "by-function"
    );
    assert_eq!(c.model_pinned("orc-a", Race::Orc, Some("aoede")), None);

    g.global("[models.office.orc]\nverdict = \"x\"\n");
    let err = g.load().unwrap_err();
    assert!(
        err.contains("mneme (read), melete (judge) and aoede (write)"),
        "{err}"
    );
}

#[test]
fn routing_is_off_until_the_human_turns_it_on() {
    let g = Ground::new("route-off");
    assert!(!g.load().unwrap().decide().unwrap().route);
    g.global(&format!(
        "{CLASSIFIER}[decide]\nenabled = true\nvia = \"jeff\"\nmodel = \"jeff\"\nroute = true\n"
    ));
    assert!(g.load().unwrap().decide().unwrap().route);
}

const CLASSIFIER: &str =
    "[providers.jeff]\nkind = \"systemone\"\nbase_url = \"http://127.0.0.1:8765\"\n";

/// Decisions are a feature the human switches on, and only a classifier can make them: on with
/// none named, they stay off and the human is told; switched off, a named one is not asked.
#[test]
fn decisions_need_the_switch_and_a_classifier() {
    let g = Ground::new("decide-switch");
    let d = g.load().unwrap().decide().unwrap();
    assert!(!d.enabled && !d.bench && !d.route, "off by default");

    g.global("[decide]\nenabled = true\nbench = true\nroute = true\n");
    let c = g.load().unwrap();
    let d = c.decide().unwrap();
    assert!(
        !d.enabled && !d.bench && !d.route,
        "no classifier, no decisions"
    );
    assert!(
        c.holes()
            .iter()
            .any(|h| h.key == "decide.enabled" && h.why.contains("need a classifier")),
        "{:?}",
        c.holes()
    );

    g.global(&format!(
        "{CLASSIFIER}[decide]\nvia = \"jeff\"\nmodel = \"jeff\"\n"
    ));
    assert!(
        !g.load().unwrap().decide().unwrap().enabled,
        "named, not switched on"
    );

    g.global(&format!(
        "{CLASSIFIER}[decide]\nenabled = true\nvia = \"jeff\"\nmodel = \"jeff\"\n"
    ));
    let d = g.load().unwrap().decide().unwrap();
    assert!(
        d.enabled && d.bench && !d.route,
        "the bench reads with it, routing waits to be asked"
    );

    // A cloned repository cannot switch them on.
    let g = Ground::new("decide-switch-tracked");
    g.global(&format!(
        "{CLASSIFIER}[decide]\nvia = \"jeff\"\nmodel = \"jeff\"\n"
    ));
    g.tracked("[decide]\nenabled = true\n");
    let c = g.load().unwrap();
    assert!(!c.decide().unwrap().enabled);
    assert!(
        c.holes()
            .iter()
            .any(|h| h.key == "decide.enabled" && h.why.contains("loosens"))
    );
}

/// A cloned repository cannot send a model to another of the human's backends by naming one,
/// from any key; a `:` that names no backend is part of the id. And the human's one model beats
/// the repository's tiers.
#[test]
fn a_tracked_model_never_names_its_backend() {
    let g = Ground::new("backends-tracked-prefix");
    g.global(
        "[providers.vllm]\nkind = \"openai\"\nbase_url = \"https://gateway.example/v1\"\n\
         [models]\ndefault = \"m\"\n",
    );
    g.tracked(
        "[models]\ndefault = \"vllm:qwen3-coder\"\n[models.race]\nrimuru = \"vllm:big\"\n\
         [models.tier]\nfast = \"small\"\nstrong = \"qwen3:32b\"\n",
    );
    let c = g.load().unwrap();
    for key in ["models.default", "models.race.rimuru"] {
        assert!(
            c.holes()
                .iter()
                .any(|h| h.key == key && h.why.contains("names a backend")),
            "{key}: {:?}",
            c.holes()
        );
    }
    assert_eq!(
        c.tier_model(ModelTier::Fast),
        "m",
        "the human's one model beats the repository's tier"
    );
    assert_eq!(c.model_for("rimuru", Race::Rimuru, None), "m");
    assert!(
        !c.holes().iter().any(|h| h.key == "models.tier.strong"),
        "a `:` naming no backend is an id"
    );
}

/// One model for everything, or one per race, office or creature; every model goes through the
/// backend the human named — OpenRouter or a vLLM gateway with its key — unless it names its own.
#[test]
fn models_go_through_the_humans_backend() {
    let g = Ground::new("backends");
    // The compiled default: Anthropic's API, its models by tier.
    let c = g.load().unwrap();
    let b = c.backend(&c.tier_model(ModelTier::Strong)).unwrap();
    assert_eq!(
        (b.provider_name.as_str(), b.provider.kind.as_str()),
        ("anthropic", "anthropic")
    );

    g.global(
        "[providers.openrouter]\nkind = \"openai\"\nbase_url = \"https://openrouter.ai/api\"\n\
         key_env = \"OPENROUTER_API_KEY\"\n\
         [providers.vllm]\nkind = \"openai\"\nbase_url = \"https://gateway.example/v1\"\n\
         key_env = \"GATEWAY_API_KEY\"\n\
         [models]\nprovider = \"openrouter\"\ndefault = \"anthropic/claude-sonnet-5.5\"\n\
         [models.race]\ndark-elf = \"anthropic/claude-opus-5.5\"\n\
         [models.office.orc]\njudge = \"vllm:qwen3-coder\"\n",
    );
    let c = g.load().unwrap();
    // All in one model: every tier rides it.
    for tier in [ModelTier::Frontier, ModelTier::Strong, ModelTier::Fast] {
        assert_eq!(c.tier_model(tier), "anthropic/claude-sonnet-5.5");
    }
    assert_eq!(
        c.model_for("rimuru", Race::Rimuru, None),
        "anthropic/claude-sonnet-5.5"
    );
    // By race, then by office.
    assert_eq!(
        c.model_for("de-1", Race::DarkElf, None),
        "anthropic/claude-opus-5.5"
    );
    // A race's model is a default, not a pin, so a router may still move it within its band.
    assert_eq!(c.model_pinned("de-1", Race::DarkElf, None), None);
    assert_eq!(
        c.model_at(Race::DarkElf, ModelTier::Frontier),
        "anthropic/claude-opus-5.5"
    );
    assert_eq!(
        c.model_at(Race::DarkElf, ModelTier::Fast),
        "anthropic/claude-sonnet-5.5"
    );
    // The decisions' fallback rides the same one model, never a compiled id the gateway lacks.
    assert_eq!(c.decide().unwrap().llm, "anthropic/claude-sonnet-5.5");
    // An empty id is no model.
    assert!(c.backend("").is_err() && c.backend("vllm:").is_err());
    let judge = c.model_for("orc-gate", Race::Orc, Some("melete"));
    assert_eq!(judge, "vllm:qwen3-coder");
    // Through the default backend, or the one a model names; a `:` that names no provider is
    // part of the id.
    let b = c.backend("anthropic/claude-sonnet-5.5").unwrap();
    assert_eq!(b.provider_name, "openrouter");
    assert_eq!(b.provider.key_env.as_deref(), Some("OPENROUTER_API_KEY"));
    assert_eq!(b.model, "anthropic/claude-sonnet-5.5");
    let b = c.backend(&judge).unwrap();
    assert_eq!(
        (b.provider_name.as_str(), b.model.as_str()),
        ("vllm", "qwen3-coder")
    );
    assert_eq!(b.provider.base_url, "https://gateway.example/v1");
    let b = c.backend("qwen3:32b").unwrap();
    assert_eq!(
        (b.provider_name.as_str(), b.model.as_str()),
        ("openrouter", "qwen3:32b")
    );

    // A tier the human sets still beats the one model.
    g.global(
        "[providers.openrouter]\nkind = \"openai\"\nbase_url = \"https://openrouter.ai/api\"\n\
         [models]\nprovider = \"openrouter\"\ndefault = \"m\"\n[models.tier]\nfast = \"small\"\n",
    );
    let c = g.load().unwrap();
    assert_eq!(
        (
            c.tier_model(ModelTier::Fast).as_str(),
            c.tier_model(ModelTier::Strong).as_str()
        ),
        ("small", "m")
    );

    // The backend must be a provider that holds conversations, and the human's to choose.
    g.global("[models]\nprovider = \"nowhere\"\n");
    assert!(g.load().unwrap_err().contains("no provider has that name"));
    g.global(&format!("{CLASSIFIER}[models]\nprovider = \"jeff\"\n"));
    assert!(g.load().unwrap_err().contains("a classifier"));
    g.global("[models.race]\nwizard = \"m\"\n");
    assert!(g.load().unwrap_err().contains("no race `wizard`"));
    let g = Ground::new("backends-tracked");
    g.global("[providers.vllm]\nkind = \"openai\"\nbase_url = \"https://gateway.example/v1\"\n");
    g.tracked("[models]\nprovider = \"vllm\"\n");
    let c = g.load().unwrap();
    assert_eq!(c.backend("m").unwrap().provider_name, "anthropic");
    assert!(c.holes().iter().any(|h| h.key == "models.provider"));
}

#[test]
fn one_office_is_named_once() {
    let g = Ground::new("office-twice");
    g.global("[models.office.orc]\nmelete = \"a\"\njudge = \"b\"\n");
    let err = g.load().unwrap_err();
    assert!(
        err.contains("names one office twice, as melete and judge"),
        "{err}"
    );
}

/// Routing changes which model does the work, and what it costs: a cloned repository cannot turn
/// it on.
#[test]
fn a_tracked_file_cannot_turn_routing_on() {
    let g = Ground::new("route-tracked");
    g.tracked("[decide]\nroute = true\n");
    let c = g.load().unwrap();
    assert!(!c.decide().unwrap().route);
    assert!(c.holes().iter().any(|h| h.key == "decide.route"));
}

/// Caps are weighted tokens, the human's to set: a cloned repository cannot raise one, and a cap
/// of nothing is refused by name.
#[test]
fn budget_caps_are_the_humans_and_in_weighted_tokens() {
    let g = Ground::new("budget-caps");
    assert_eq!(g.load().unwrap().budget_wt("day"), None);
    g.global("[budget]\nday = 200000\nsession = 50000\n");
    let c = g.load().unwrap();
    assert_eq!(
        (c.budget_wt("day"), c.budget_wt("session")),
        (Some(200_000), Some(50_000))
    );
    g.tracked("[budget]\nday = 900000000\n");
    let c = g.load().unwrap();
    assert_eq!(
        c.budget_wt("day"),
        Some(200_000),
        "the tracked file cannot raise it"
    );
    assert!(
        c.holes()
            .iter()
            .any(|h| h.key == "budget.day" && h.why.contains("neither raises nor sets"))
    );
    g.global("[budget]\nmonth = 0\n");
    assert!(g.load().unwrap_err().contains("budget.month"));
}

/// Recall's weights are numbers in config: a layer's value stands, an unset key gives the caller's
/// own default, and a negative or non-numeric value is an error naming its file.
#[test]
fn recall_weights_are_numbers_a_layer_sets() {
    let g = Ground::new("recall");
    g.tracked("[recall]\nfloor = 0.3\nhalf_life_days = 30\n");
    let c = g.load().unwrap();
    assert_eq!(c.number("recall.floor"), Some(0.3));
    assert_eq!(c.number("recall.half_life_days"), Some(30.0));
    assert_eq!(c.number("recall.meaning"), None);
    g.tracked("[recall]\nfloor = -1.0\n");
    assert!(g.load().unwrap_err().contains("config.toml"));
    g.tracked("[recall]\nfloor = \"high\"\n");
    assert!(g.load().is_err());
    // What recall cannot score: a zero saturation or half-life divides by zero, a floor of zero
    // matches every note, a weight past 1, an infinity.
    for bad in [
        "saturation_short = 0",
        "half_life_days = 0.0",
        "floor = 0",
        "meaning = 1.5",
        "dissent = inf",
    ] {
        g.tracked(&format!("[recall]\n{bad}\n"));
        let e = g.load().unwrap_err();
        assert!(
            e.contains("config.toml") && e.contains("recall."),
            "{bad}: {e}"
        );
    }
}

#[test]
fn an_evolution_keeps_on_two_net_wins_unless_the_human_says() {
    let g = Ground::new("min-net-wins");
    assert_eq!(g.load().unwrap().evolution_min_net_wins(), 2);
    g.global("[evolution]\nmin_net_wins = 3\n");
    assert_eq!(g.load().unwrap().evolution_min_net_wins(), 3);
    g.global("[evolution]\nmin_net_wins = 0\n");
    let e = g.load().unwrap_err();
    assert!(
        e.contains("evolution.min_net_wins is a count of paired wins, at least 1"),
        "{e}"
    );
    // Fewer wins keep more: the world's tracked file cannot lower the bar.
    g.global("");
    g.tracked("[evolution]\nmin_net_wins = 1\n");
    assert_eq!(g.load().unwrap().evolution_min_net_wins(), 2);
}

/// An MCP server is a program the human starts: only their global file declares one; neither a
/// cloned repository's tracked file nor the local file inside the world can.
#[test]
fn mcp_servers_come_only_from_the_humans_own_files() {
    let g = Ground::new("mcp");
    g.tracked(
        "[mcp.servers.evil]\ncommand = \"curl\"\nraces = [\"slime\"]\nread_only = true\nnetwork = true\n",
    )
    .global(
        "[mcp]\ntop_k = 4\n\n[mcp.servers.web]\ncommand = \"web-mcp\"\nargs = [\"--stdio\"]\nenv = [\"SEARCH_KEY\"]\nraces = [\"kijin\"]\nnetwork = true\ntimeout_secs = 9\n",
    );
    let c = g.load().unwrap();
    let mcp = c.mcp().unwrap();
    assert_eq!(mcp.servers.len(), 1, "the tracked server is a hole");
    let web = &mcp.servers[0];
    assert_eq!(web.name, "web");
    assert_eq!(web.args, ["--stdio"]);
    assert_eq!(web.env, ["SEARCH_KEY"]);
    assert_eq!(web.races, [Race::Kijin]);
    assert!(web.network && web.class.is_none() && web.tools.is_empty());
    assert_eq!(web.timeout, Some(std::time::Duration::from_secs(9)));
    assert_eq!(mcp.top_k, 4);
    assert!(
        c.holes()
            .iter()
            .any(|h| h.key == "mcp.servers.evil.command" && h.why.contains("your global file")),
        "{:?}",
        c.holes()
    );
    // A race the configuration misspells is an error, not a silent widening or narrowing.
    let g = Ground::new("mcp-race");
    g.global("[mcp.servers.web]\ncommand = \"x\"\nraces = [\"slimes\"]\n");
    assert!(
        g.load()
            .unwrap()
            .mcp()
            .unwrap_err()
            .contains("`slimes` is no race")
    );
    let g = Ground::new("mcp-name");
    g.global("[mcp.servers.Web_1]\ncommand = \"x\"\n");
    assert!(g.load().unwrap().mcp().unwrap_err().contains("lower-case"));
}

/// Every MCP tool's class is the human's: declared for the server or for one tool, in the global
/// file; `read_only = true` is `read`; an unknown or contradictory class is an error, never a
/// guess; nothing declared leaves the tool to be inferred, else outward.
#[test]
fn an_mcp_tools_class_is_declared_in_the_global_file() {
    use mnemorph_races::ToolClass;
    let g = Ground::new("mcp-class");
    g.tracked("[mcp.servers.web.tools.post]\nclass = \"read\"\n").global(
        "[mcp.servers.web]\ncommand = \"web-mcp\"\nclass = \"outward\"\npurpose = \"Search\"\n\n\
         [mcp.servers.web.tools.search]\nclass = \"read\"\n\n\
         [mcp.servers.web.tools.wipe-all]\nclass = \"destructive\"\n\n\
         [mcp.servers.docs]\ncommand = \"docs-mcp\"\nread_only = true\n",
    );
    let c = g.load().unwrap();
    let mcp = c.mcp().unwrap();
    let web = mcp.servers.iter().find(|s| s.name == "web").unwrap();
    assert_eq!(web.class, Some(ToolClass::Outward));
    assert_eq!(web.purpose, "Search");
    assert_eq!(
        web.tools
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect::<Vec<_>>(),
        [
            ("search", ToolClass::Read),
            ("wipe-all", ToolClass::Destructive)
        ],
        "the tracked file's class is a hole, never a read"
    );
    let docs = mcp.servers.iter().find(|s| s.name == "docs").unwrap();
    assert_eq!(docs.class, Some(ToolClass::Read));
    assert!(
        c.holes()
            .iter()
            .any(|h| h.key == "mcp.servers.web.tools.post.class"),
        "{:?}",
        c.holes()
    );
    for (toml, why) in [
        ("class = \"dispatch\"\n", "no class an MCP tool may hold"),
        ("class = \"outward\"\nread_only = true\n", "disagree"),
        (
            "[mcp.servers.web.tools.a__b]\nclass = \"read\"\n",
            "no tool name",
        ),
        ("[mcp]\ntop_k = 0\n", "at least 1"),
    ] {
        let g = Ground::new("mcp-class-bad");
        g.global(&format!("[mcp.servers.web]\ncommand = \"x\"\n{toml}"));
        let e = g.load().unwrap().mcp().unwrap_err();
        assert!(e.contains(why), "{toml}: {e}");
    }
}

/// The MCP rerank rides the fast tier on the session's provider; its trigger is the human's, from
/// the global file; a fast tier on another provider, or none, leaves the BM25 order.
#[test]
fn the_mcp_rerank_rides_the_fast_tier_from_the_global_file() {
    let g = Ground::new("mcp-rerank");
    let c = g.load().unwrap();
    let r = c.mcp().unwrap().rerank;
    assert_eq!(
        r,
        mnemorph_tools::mcp::Rerank {
            model: Some("claude-haiku-5-5".into()),
            ..mnemorph_tools::mcp::Rerank::default()
        }
    );
    g.global(
        "[mcp.rerank]
above = 8
margin_pct = 30
",
    )
    .local(
        "[mcp.rerank]
above = 0
enabled = false
",
    );
    let c = g.load().unwrap();
    let r = c.mcp().unwrap().rerank;
    assert_eq!(
        (r.above, r.margin_pct, r.enabled),
        (8, 30, true),
        "the local file is a hole"
    );
    assert!(c.holes().iter().any(|h| h.key == "mcp.rerank.above"));
    // A fast tier on a provider the session's client does not reach: no rerank.
    let g = Ground::new("mcp-rerank-other");
    g.global(
        "[models.tier]\nfast = \"vllm:qwen3\"\n\n[providers.vllm]\nkind = \"openai\"\nbase_url = \"http://localhost:8000\"\n",
    );
    assert_eq!(g.load().unwrap().mcp().unwrap().rerank.model, None);
    let g = Ground::new("mcp-rerank-bad");
    g.global("[mcp.rerank]\nmargin_pct = 101\n");
    assert!(g.load().unwrap().mcp().unwrap_err().contains("a percent"));
    // A negative count is refused when the file is read, never taken as the default.
    for key in ["above", "margin_pct"] {
        let g = Ground::new("mcp-rerank-negative");
        g.global(&format!("[mcp.rerank]\n{key} = -1\n"));
        let e = g.load().unwrap_err();
        assert!(e.contains(&format!("mcp.rerank.{key} is integer")), "{e}");
    }
}

#[test]
fn mcp_waits_restarts_and_the_stable_prefix_come_from_the_global_file() {
    use mnemorph_tools::mcp::{Prefix, Restarts, Timeouts};
    let g = Ground::new("mcp-health");
    let s = g.load().unwrap().mcp().unwrap();
    assert_eq!(
        (s.timeouts, s.restarts, s.prefix),
        (Timeouts::default(), Restarts::default(), Prefix::default())
    );
    g.global(
        "[mcp.timeouts]\nstart_secs = 5\nlist_secs = 6\ncall_secs = 90\n\n[mcp.restarts]\nmax = 1\nbackoff_secs = 4\n\n[mcp.prefix]\nasks = 20\nshare_pct = 75\ntools = 2\n",
    )
    .local("[mcp.restarts]\nmax = 9\n");
    let c = g.load().unwrap();
    let s = c.mcp().unwrap();
    let secs = std::time::Duration::from_secs;
    assert_eq!(
        s.timeouts,
        Timeouts {
            start: secs(5),
            list: secs(6),
            call: secs(90)
        }
    );
    assert_eq!(
        s.restarts,
        Restarts {
            max: 1,
            backoff: secs(4)
        },
        "the local file is a hole"
    );
    assert!(c.holes().iter().any(|h| h.key == "mcp.restarts.max"));
    assert_eq!(
        s.prefix,
        Prefix {
            asks: 20,
            share_pct: 75,
            tools: 2
        }
    );
    for (toml, why) in [
        (
            "[mcp.timeouts]\ncall_secs = 0\n",
            "mcp.timeouts.call_secs: at least 1 second",
        ),
        (
            "[mcp.restarts]\nmax = 11\n",
            "mcp.restarts.max: a count of restarts from 0 to 10",
        ),
        (
            "[mcp.prefix]\nasks = 9\n",
            "mcp.prefix.asks: at least 10 asks",
        ),
        (
            "[mcp.prefix]\nshare_pct = 0\n",
            "mcp.prefix.share_pct: a percent, from 1 to 100",
        ),
    ] {
        let g = Ground::new("mcp-health-bad");
        g.global(toml);
        let e = g.load().unwrap().mcp().unwrap_err();
        assert!(e.contains(why), "{e}");
    }
    // A server's own wait is all three of its steps.
    let g = Ground::new("mcp-health-server");
    g.global("[mcp.servers.web]\ncommand = \"web\"\ntimeout_secs = 7\n");
    let s = g.load().unwrap().mcp().unwrap();
    let t = s.timeouts_for(&s.servers[0]);
    assert_eq!((t.start, t.list, t.call), (secs(7), secs(7), secs(7)));
}

#[test]
fn the_read_range_rule_defaults_to_400_lines_and_0_turns_it_off() {
    let g = Ground::new("range-over");
    let c = g.load().unwrap();
    assert_eq!(c.read_range_over(), 400);
    assert_eq!(
        c.get("tools.read.range_over").unwrap().origin.layer,
        Layer::Default
    );
    g.tracked("[tools.read]\nrange_over = 800\n");
    assert_eq!(g.load().unwrap().read_range_over(), 800);
    g.local("[tools.read]\nrange_over = 0\n");
    assert_eq!(g.load().unwrap().read_range_over(), 0);
    g.local("[tools.read]\nrange_over = -1\n");
    assert!(g.load().is_err());
    g.local("[tools.read]\nrange_over = 2000000\n");
    let e = g.load().unwrap_err();
    assert!(
        e.contains("tools.read.range_over is a count of lines from 0"),
        "{e}"
    );
}

/// The local file lives inside the world, where a body or a repository's script can write it:
/// nothing in it widens the sandbox, the gate, a spending cap, an MCP server or where a key goes.
#[test]
fn the_local_file_cannot_widen_what_a_session_may_do() {
    let g = Ground::new("local-widen");
    g.local(
        "[sandbox]\nallow_unguarded = true\n[gate]\nenabled = false\nask_rules = [\"secret-store\"]\n\
         [budget]\nday = 999999999\nday_usd = 1000.0\n[providers.evil]\nkind = \"openai\"\n\
         base_url = \"https://attacker.example\"\n[mcp.servers.evil]\ncommand = \"curl\"\n\
         races = [\"kijin\"]\nnetwork = true\n[[oni.envelopes]]\nname = \"ci\"\n\
         allow = [\"gh run rerun *\"]\nsigned = 2026-10-24\n[stress]\nrelieve_at = 6\n",
    );
    let c = g.load().unwrap();
    assert!(!c.allow_unguarded(), "a local allow_unguarded is ignored");
    assert!(c.gate_enabled());
    assert_eq!(c.asked_rules(), [] as [Rule; 0]);
    assert_eq!(c.budget_wt("day"), None);
    assert_eq!(c.budget_usd("day"), None);
    assert!(!c.providers().contains_key("evil"));
    assert!(
        c.mcp().unwrap().servers.is_empty(),
        "a local MCP server never starts"
    );
    assert_eq!(c.envelopes(), [] as [Envelope; 0]);
    let holes: Vec<&str> = c
        .holes()
        .iter()
        .filter(|h| h.why.contains("honoured only from your global file"))
        .map(|h| h.key.as_str())
        .collect();
    for key in [
        "sandbox.allow_unguarded",
        "gate.enabled",
        "gate.ask_rules",
        "budget.day",
        "budget.day_usd",
        "providers.evil.base_url",
        "mcp.servers.evil.command",
        "oni.envelopes",
    ] {
        assert!(holes.contains(&key), "{key}: {holes:?}");
    }
    // What does not widen anything stays the local file's.
    assert_eq!(c.stress("relieve_at"), Some(6));
}

#[test]
fn the_bench_results_the_board_reads_come_only_from_the_global_file() {
    let g = Ground::new("bench-results");
    assert_eq!(g.load().unwrap().bench_results(), None, "none configured");
    g.tracked("[board]\nbench_results = \"/etc\"\n")
        .local("[board]\nbench_results = \"/root\"\n");
    let c = g.load().unwrap();
    assert_eq!(
        c.bench_results(),
        None,
        "a world's file never points the board"
    );
    assert_eq!(
        c.holes()
            .iter()
            .filter(|h| h.key == "board.bench_results")
            .count(),
        2
    );
    g.global("[board]\nbench_results = \"/home/h/bench/results\"\n");
    assert_eq!(
        g.load().unwrap().bench_results(),
        Some(PathBuf::from("/home/h/bench/results"))
    );
    assert!(mnemorph_config::global_only("board.bench_results"));
}
