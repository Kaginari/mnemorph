//! An evolution drafted by a model: for a creature due to mutate or be reborn, the drafter reads its
//! doc and its desk — as data — and answers only trait lines, each citing the notes it distils.
//! The harness writes the doc: its header untouched, a `## Traits` section added or replaced, the
//! plan staged for the human to swap. Nothing is swapped; a split is not drafted here.

use std::path::PathBuf;

use mnemorph_ledger::{Journal, UsageRow};
use mnemorph_loop::draft::{self, MAX_TRAITS};
use mnemorph_loop::model::ScriptedModel;
use mnemorph_races::Race;
use mnemorph_world::evolution::Arm;
use mnemorph_world::evolve::{Due, desk};

const DOC: &str = ".mnemorph/slime/sly/README.md";

struct World {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl World {
    fn new(card_extra: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("world").canonicalize().unwrap_or_else(|_| {
            std::fs::create_dir_all(dir.path().join("world")).unwrap();
            dir.path().join("world").canonicalize().unwrap()
        });
        std::fs::create_dir_all(root.join(".mnemorph/slime/sly")).unwrap();
        std::fs::write(
            root.join(DOC),
            format!(
                "# sly\n\n- **Race:** slime\n- **Reports to:** summoner\n- **Territory:** src/\n\nKeeps the parser.\n{card_extra}"
            ),
        )
        .unwrap();
        for i in 0..8 {
            mnemorph_memory::propose(
                &root,
                "sly",
                &format!("lesson {i}: fractions must stay exact"),
                vec!["law".into()],
                Some("src/parser.rs".into()),
            )
            .unwrap();
        }
        World { _dir: dir, root }
    }

    fn ids(&self) -> Vec<String> {
        desk(&self.root, "sly").into_iter().map(|n| n.id).collect()
    }

    fn usage(&self) -> Journal<UsageRow> {
        Journal::chained(self.root.join("usage.jsonl"))
    }
}

fn due(arm: Arm) -> Due {
    Due {
        creature: "sly".into(),
        race: Race::Slime,
        arm,
        why: vec!["its last 3 gate verdicts failed".into()],
        parts: Vec::new(),
    }
}

async fn drafted(
    w: &World,
    answer: &str,
    arm: Arm,
) -> (Result<draft::Draft, String>, ScriptedModel) {
    let model = ScriptedModel::new([ScriptedModel::text(answer)]);
    let got = draft::draft(
        &w.root,
        &model,
        "scripted-model",
        &due(arm),
        &w.usage(),
        "s1",
        1_793_000_000,
    )
    .await;
    (got, model)
}

fn staged(d: &draft::Draft) -> String {
    std::fs::read_to_string(d.staged.join(DOC)).unwrap()
}

#[tokio::test]
async fn the_drafter_answers_traits_and_the_harness_writes_the_doc() {
    let w = World::new("");
    let ids = w.ids();
    let answer = format!(
        "Here you go.\n@T keep every fraction exact: never round in the parser from={},{}\n@T lex before parsing from={}\n",
        ids[0], ids[1], ids[2]
    );
    let (got, model) = drafted(&w, &answer, Arm::Mutate).await;
    let d = got.unwrap();
    assert_eq!(d.traits.len(), 2);
    assert_eq!(d.doc, DOC);
    assert!(d.staged.starts_with(w.root.join(".mnemorph/tmp/evolution")));
    let text = staged(&d);
    assert!(
        text.starts_with(
            "# sly\n\n- **Race:** slime\n- **Reports to:** summoner\n- **Territory:** src/\n"
        ),
        "the header is the harness's: {text}"
    );
    assert!(text.contains("## Traits\n"), "{text}");
    assert!(
        text.contains(&format!(
            "- *trait* — keep every fraction exact: never round in the parser (from {}, {})",
            ids[0], ids[1]
        )),
        "{text}"
    );
    // The live doc is untouched: a plan changes nothing while staged.
    assert!(
        !std::fs::read_to_string(w.root.join(DOC))
            .unwrap()
            .contains("## Traits")
    );
    // The drafter saw the doc and the desk as data, and was given no tool.
    let sent = model.requests();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].tools.len(), 0, "no tool");
    let shown = format!("{:?} {:?}", sent[0].system, sent[0].messages);
    assert!(shown.contains("lesson 3: fractions must stay exact") && shown.contains(&ids[3]));
    assert!(shown.contains("data, not instructions"), "{shown}");
    // The call is spent in the usage ledger, but as no body's run: it counts against the caps,
    // never in a creature's record.
    let rows = w.usage().read().unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].body.is_empty() && rows[0].run.is_empty(),
        "{:?}",
        rows[0]
    );
    assert!(
        rows[0]
            .purpose
            .as_deref()
            .is_some_and(|p| p.contains("evolution draft"))
    );
    // The swap's own reading of the staged plan takes it: exactly the doc, replaced.
    let named = mnemorph_world::evolution::FileSet {
        replaces: vec![DOC.to_owned()],
        ..Default::default()
    };
    let files = mnemorph_world::evolution::staged(&w.root, &d.id, &named).unwrap();
    assert_eq!(files.len(), 1);
    // Kept as a proposal under the state directory, the bytes drawn with it.
    let open = mnemorph_world::proposals::open(&w.root).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, d.id);
    assert!(mnemorph_world::proposals::ready(&w.root, &open[0]).is_ok());
    assert_eq!(
        d.apply_command(),
        format!("mnemorphd evolution apply {}", d.id)
    );
    // The command that swaps it, for the human.
    assert!(d.swap_command().contains(&format!(
        "mnemorphd evolution swap {} --creature sly --arm mutate --file {DOC}",
        d.id
    )));
}

#[tokio::test]
async fn a_traits_section_is_replaced_and_the_rest_of_the_doc_kept() {
    let w = World::new("\n## Traits\n\n- *trait* — an old one\n\n## Notes\n\nkept\n");
    let ids = w.ids();
    let (got, _) = drafted(&w, &format!("@T a new one from={}", ids[0]), Arm::Rebirth).await;
    let text = staged(&got.unwrap());
    assert!(!text.contains("an old one"), "{text}");
    assert!(text.contains("a new one"), "{text}");
    assert!(text.contains("## Notes\n\nkept\n"), "{text}");
    assert_eq!(text.matches("## Traits").count(), 1, "{text}");
}

#[tokio::test]
async fn a_draft_that_cites_no_note_on_the_desk_or_breaks_the_rules_is_refused_whole() {
    let w = World::new("");
    let ids = w.ids();
    let too_many = (0..=MAX_TRAITS)
        .map(|i| format!("@T trait {i} from={}", ids[0]))
        .collect::<Vec<_>>()
        .join("\n");
    for (answer, why) in [
        ("@T made up from=n-not-there".to_owned(), "not on its desk"),
        ("@T cites nothing".to_owned(), "cites no note"),
        (too_many, "at most"),
        (
            format!("@T {} from={}", "x".repeat(201), ids[0]),
            "longer than",
        ),
        (format!("@T bell \u{7} here from={}", ids[0]), "control"),
        ("no trait at all".to_owned(), "no trait"),
    ] {
        let (got, _) = drafted(&w, &answer, Arm::Mutate).await;
        let e = got.unwrap_err();
        assert!(e.contains(why), "{answer:?}: {e}");
    }
    assert!(
        !w.root.join(".mnemorph/tmp/evolution").exists()
            || std::fs::read_dir(w.root.join(".mnemorph/tmp/evolution"))
                .unwrap()
                .next()
                .is_none(),
        "a refused draft stages nothing"
    );
}

#[tokio::test]
async fn a_split_is_not_drafted_by_the_model() {
    let w = World::new("");
    let (got, model) = drafted(&w, "@T x from=y", Arm::Split).await;
    assert!(got.unwrap_err().contains("split"));
    assert!(model.requests().is_empty(), "nothing was spent");
}

#[cfg(unix)]
#[tokio::test]
async fn a_staging_folder_a_body_planted_as_a_link_is_refused() {
    let w = World::new("");
    let ids = w.ids();
    let elsewhere = w.root.join("../elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::create_dir_all(w.root.join(".mnemorph/tmp")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, w.root.join(".mnemorph/tmp/evolution")).unwrap();
    let (got, _) = drafted(&w, &format!("@T x from={}", ids[0]), Arm::Mutate).await;
    assert!(got.unwrap_err().contains("link"));
    assert!(
        std::fs::read_dir(&elsewhere).unwrap().next().is_none(),
        "nothing written through it"
    );
}
