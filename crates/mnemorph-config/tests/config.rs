//! The configuration's lessons, each as a test.

use std::fs;
use std::path::PathBuf;

use mnemorph_config::{Config, Layer, Paths, set};
use mnemorph_races::Race;
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
    g.global("[models.creature]\nslime-wire = \"model-a\"\n[budget]\nday_usd = 5\n")
        .tracked("[models.creature]\norc-gate = \"model-b\"\n[stress]\nescalate_at = 4\n")
        .local("[budget]\nday_usd = 2.5\n[gate]\nask_rules = [\"in-place-edit\"]\n");
    let c = g.load().unwrap();
    let paths = g.paths();
    let at = |key: &str| c.get(key).unwrap().origin.clone();

    assert_eq!(at("gate.timeout_secs").layer, Layer::Default);
    assert_eq!(at("gate.timeout_secs").file, None);
    assert_eq!(
        at("budget.day_usd").file.as_deref(),
        Some(paths.local.as_path()),
        "local overrides global"
    );
    assert_eq!(
        at("stress.escalate_at").file.as_deref(),
        Some(paths.tracked.as_path())
    );
    assert_eq!(
        at("gate.ask_rules").file.as_deref(),
        Some(paths.local.as_path()),
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
    assert_eq!(c.budget_usd("day"), Some(2.5));
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
        "[gate]\nenabled = false\nask_rules = [\"secret-store\"]\napprove = [\"outward\"]\n[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\n",
    );
    let c = g.load().unwrap();
    assert!(c.gate_enabled());
    assert_eq!(c.asked_rules(), [] as [mnemorph_tools::Rule; 0]);
    assert_eq!(c.envelopes(), [] as [mnemorph_config::Envelope; 0]);
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
            "oni.envelopes"
        ]
    );

    // The same keys from the human's own files are honoured.
    g.local("[gate]\nask_rules = [\"secret-store\"]\n[[oni.envelopes]]\nname = \"ci\"\nallow = [\"gh run rerun *\"]\n");
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
    g.local("[gate]\nask_rules = [\"root-path\"]\n");
    assert!(g.load().unwrap_err().contains("on the floor"));
    g.local("[gate]\nask_rules = [\"anything-goes\"]\n");
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
        g.local(&format!(
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
        g.local(&format!(
            "[[oni.envelopes]]\nname = \"e\"\nallow = [\"{pattern}\"]\n"
        ));
        let e = g.load().unwrap_err();
        assert!(e.contains("admits"), "{pattern}: {e}");
    }
    g.local("[[oni.envelopes]]\nname = \"feat\"\nallow = [\"git push origin feat/*\", \"gh run rerun *\"]\n");
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
    let path = g.paths().local;
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
    g.local("[providers.x]\nkind = \"openai\"\nbase_url = \"http://localhost:8000\"\n");
    assert!(
        g.load().unwrap().providers().contains_key("x"),
        "the human's own file is honoured"
    );
}
