use super::*;

const FIELDS: [&str; 5] = [
    "\nInvariant: ",
    "\nLook: ",
    "\nCheck: ",
    "\nWitness: ",
    "\nNot a finding: ",
];

#[test]
fn every_class_has_its_playbook() {
    for class in AttackClass::ALL {
        let text = class.playbook();
        assert!(
            text.starts_with(&format!("### {}\n", class.name())),
            "{class}: heading"
        );
        let mut at = 0;
        for field in FIELDS {
            let found = text[at..]
                .find(field)
                .unwrap_or_else(|| panic!("{class}: {field:?} in order"));
            at += found + field.len();
        }
        assert_eq!(text.matches("\n### ").count(), 0, "{class}: one heading");
        let [owner] = class.owners()[..] else {
            panic!("{class}: one owner");
        };
        assert_eq!(playbook(owner, class), Some(text));
        for other in Speciality::ALL.into_iter().filter(|&s| s != owner) {
            assert_eq!(
                playbook(other, class),
                None,
                "{other} does not hold {class}"
            );
        }
    }
}

#[test]
fn the_size_caps_hold() {
    for class in AttackClass::ALL {
        let len = class.playbook().len();
        assert!(len <= MAX_CLASS, "{class}: {len} bytes over {MAX_CLASS}");
    }
    for s in Speciality::ALL {
        let len: usize = s.classes().iter().map(|c| c.playbook().len()).sum();
        assert!(
            len <= MAX_SPECIALITY,
            "{s}: {len} bytes over {MAX_SPECIALITY}"
        );
        assert!(brief(s, s.surfaces()).len() <= MAX_SPECIALITY + 64, "{s}");
    }
}

/// The playbooks carry none of what `wire-forgery` hunts: no control character but the newline,
/// nothing invisible, nothing outside ASCII.
#[test]
fn the_playbooks_are_plain_text() {
    for class in AttackClass::ALL {
        for (i, ch) in class.playbook().char_indices() {
            assert!(
                ch == '\n' || (ch.is_ascii() && !ch.is_ascii_control()),
                "{class}: {ch:?} at byte {i}"
            );
        }
    }
}

#[test]
fn every_compiled_surface_maps_to_owned_classes() {
    for s in Speciality::HUNTERS {
        for surface in s.surfaces() {
            let classes = relevant(surface).unwrap_or_else(|| panic!("{s}: {surface} unmapped"));
            assert!(!classes.is_empty(), "{s}: {surface}");
            for c in classes {
                assert!(
                    s.classes().contains(c),
                    "{s}: {surface} maps {c}, not its own"
                );
            }
        }
        for c in s.classes() {
            assert!(
                s.surfaces()
                    .iter()
                    .any(|g| relevant(g).is_some_and(|cs| cs.contains(c))),
                "{s}: {c} is reached by no surface"
            );
        }
    }
    assert_eq!(relevant("src/auth/"), None);
}

#[test]
fn a_landing_loads_only_the_owned_and_relevant() {
    let names = |loaded: Vec<(AttackClass, &str)>| -> Vec<&str> {
        loaded
            .into_iter()
            .map(|(c, text)| {
                assert_eq!(text, c.playbook());
                c.name()
            })
            .collect()
    };
    assert_eq!(
        names(for_landing(
            Speciality::Harness,
            &["crates/mnemorph-tui/**"]
        )),
        ["approval-binding", "terminal-escape"]
    );
    // Two surfaces: their union, each class once, in the speciality's order.
    assert_eq!(
        names(for_landing(
            Speciality::Harness,
            &[
                "crates/mnemorph-tui/**",
                "crates/mnemorph-config/src/outward.rs"
            ]
        )),
        [
            "command-injection",
            "approval-binding",
            "terminal-escape",
            "hook-laundering"
        ]
    );
    assert_eq!(
        names(for_landing(Speciality::Supply, &["**/Cargo.lock"])),
        ["known-vulnerability", "dependency-confusion"]
    );
    assert_eq!(
        names(for_landing(Speciality::Law, &["**/log.md"])),
        ["ledger-integrity"]
    );
    // A world's declared surface: nothing narrows it, so every owned class loads — and only those.
    let all_web: Vec<&str> = Speciality::Web.classes().iter().map(|c| c.name()).collect();
    assert_eq!(names(for_landing(Speciality::Web, &["src/auth/"])), all_web);
    // Another speciality's compiled glob is no compiled surface of this one.
    let all_agent: Vec<&str> = Speciality::Agent
        .classes()
        .iter()
        .map(|c| c.name())
        .collect();
    assert_eq!(
        names(for_landing(Speciality::Agent, &["crates/mnemorph-tui/**"])),
        all_agent
    );
    // The generalist owns none; no surface, no playbook.
    assert_eq!(for_landing(Speciality::General, &["src/auth/"]), []);
    assert_eq!(for_landing::<&str>(Speciality::Harness, &[]), []);
}

/// No single compiled surface loads its speciality's whole playbook: the loading narrows.
#[test]
fn one_surface_never_loads_everything() {
    for s in Speciality::HUNTERS {
        for surface in s.surfaces() {
            let loaded = for_landing(s, &[surface]);
            assert!(!loaded.is_empty(), "{s}: {surface}");
            assert!(loaded.len() < s.classes().len(), "{s}: {surface} loads all");
        }
    }
}

/// Each real finding stays in its class as a regression to check again.
#[test]
fn the_seeded_findings_are_in_their_classes() {
    let seeds: [(AttackClass, &[&str]); 10] = [
        (
            AttackClass::CredentialGrant,
            &[
                ".cargo/credentials",
                ".config/git/credentials",
                ".m2/",
                ".nvm",
                ".npmrc",
                "GOPATH",
            ],
        ),
        (
            AttackClass::Toctou,
            &["symlink swaps", "writes and deletes", "pinned dirfd"],
        ),
        (
            AttackClass::HookLaundering,
            &[
                "-c",
                "GIT_SSH_COMMAND",
                "pushurl",
                "mirror",
                "followTags",
                "short refspec",
            ],
        ),
        (
            AttackClass::WireForgery,
            &["invisible characters", "separators"],
        ),
        (AttackClass::TerminalEscape, &["cards and status"]),
        (
            AttackClass::PhantomEffect,
            &["pre-existing labels, branches", "another author"],
        ),
        (
            AttackClass::LedgerIntegrity,
            &[
                "fingerprint not recomputed",
                "hash of the command instead of its output",
            ],
        ),
        (
            AttackClass::LedgerIntegrity,
            &["hollow coverage", "uniform reasons"],
        ),
        (
            AttackClass::ApprovalBinding,
            &["swapped after approval", "PR body file"],
        ),
        (
            AttackClass::ApprovalBinding,
            &["Headless approval typeahead"],
        ),
    ];
    for (class, needles) in seeds {
        let text = class.playbook();
        let regression = text
            .split("\nRegression: ")
            .nth(1)
            .unwrap_or_else(|| panic!("{class}: no regression line"));
        for needle in needles {
            assert!(regression.contains(needle), "{class}: {needle}");
        }
    }
}

#[test]
fn the_brief_holds_the_loaded_playbooks() {
    let text = brief(Speciality::Harness, &["crates/mnemorph-tui/**"]);
    assert!(text.starts_with("## Playbooks (dark-elf-harness)\n\n### approval-binding\n"));
    assert!(text.contains("\n\n### terminal-escape\n"));
    assert!(!text.contains("### toctou"));
    assert!(text.ends_with("\n\n"));
    assert_eq!(
        brief(Speciality::Harness, &["src/auth/"])
            .matches("\n### ")
            .count(),
        8
    );
    assert_eq!(brief::<&str>(Speciality::Harness, &[]), "");
}

/// Routing hands written paths, not globs: each is placed on the compiled surfaces that hold it.
#[test]
fn written_paths_load_their_surfaces_classes() {
    let names = |s: Speciality, paths: &[&str]| -> Vec<&'static str> {
        for_landing(s, paths)
            .into_iter()
            .map(|(c, _)| c.name())
            .collect()
    };
    assert_eq!(
        names(Speciality::Harness, &["crates/mnemorph-tui/src/card.rs"]),
        ["approval-binding", "terminal-escape"]
    );
    assert_eq!(
        names(Speciality::Supply, &["./crates/mnemorph-races/Cargo.toml"]),
        ["known-vulnerability", "dependency-confusion"]
    );
    assert_eq!(
        names(Speciality::Law, &[".mnemorph/log.md"]),
        ["ledger-integrity"]
    );
    // A path two compiled surfaces hold loads both surfaces' classes.
    assert_eq!(
        names(
            Speciality::Secrets,
            &["crates/mnemorph-config/src/secrets.rs"]
        ),
        [
            "secret-exposure",
            "credential-grant",
            "redaction-gap",
            "data-isolation"
        ]
    );
    assert_eq!(
        names(
            Speciality::Harness,
            &["crates/mnemorph-config/src/runner.rs"]
        ),
        [
            "toctou",
            "command-injection",
            "approval-binding",
            "hook-laundering"
        ]
    );
    // A path no compiled surface holds lies on a declared one: every owned class.
    assert_eq!(
        names(Speciality::Web, &["api/auth.rs"]).len(),
        Speciality::Web.classes().len()
    );
    for s in Speciality::HUNTERS {
        for surface in s.surfaces() {
            let path = surface.replace("**", "x/y").replace('*', "z");
            let loaded = for_landing(s, &[path.as_str()]);
            assert!(loaded.len() < s.classes().len(), "{s}: {path} loads all");
        }
    }
}

/// The generalist owns no class; its brief is the chains it hunts across surfaces.
#[test]
fn the_generalist_is_briefed_on_chains() {
    assert!(CHAINS.starts_with("### chains\n"));
    assert!(CHAINS.len() <= MAX_CLASS, "{} bytes", CHAINS.len());
    for ch in CHAINS.chars() {
        assert!(
            ch == '\n' || (ch.is_ascii() && !ch.is_ascii_control()),
            "{ch:?}"
        );
    }
    let text = brief(
        Speciality::General,
        &[
            "crates/mnemorph-tools/src/write.rs",
            ".github/workflows/ci.yml",
        ],
    );
    assert!(text.starts_with("## Playbooks (dark-elf)\n\n### chains\n"));
    assert!(text.contains("outward act"));
    assert!(text.len() <= MAX_CLASS + 64);
    assert_eq!(brief::<&str>(Speciality::General, &[]), "");
    assert_eq!(for_landing(Speciality::General, &["src/auth/"]), []);
}

/// The law audit's checklist (#104): stop propagation, Mind provenance and the journal chains, each with the files to
/// read and the tests that witness it, loaded whenever the law is routed to their surfaces.
#[test]
fn the_law_audit_holds_its_checklist() {
    let items: [(AttackClass, &str, &[&str]); 2] = [
        (
            AttackClass::DetachedWork,
            "crates/mnemorph-loop/src/session.rs",
            &[
                "every descendant body and command",
                "never left claimed",
                "`Interrupt`",
                "process group",
                "`settle`",
                "background campaign",
                "esc_stops_every_descendant_and_releases_its_order",
                "esc_stops_a_running_command_and_keeps_its_output",
            ],
        ),
        (
            AttackClass::LedgerIntegrity,
            "crates/mnemorph-world/src/minds.rs",
            &[
                "never law",
                "doc and digest",
                "retired Mind stops loading",
                "`- **Retired:** yes`",
                "un-retire",
                "a_retired_mind_never_loads",
                "a_worn_mind_names_its_source",
                "a_mind_edited_since_adoption_does_not_load",
                "retirement_comes_from_the_witness",
                "`chains::verify`",
                "anchored",
                "a_silent_edit_is_found_at_its_row",
            ],
        ),
    ];
    for (class, surface, needles) in items {
        let text = class.playbook();
        for needle in needles {
            assert!(text.contains(needle), "{class}: {needle}");
        }
        assert!(
            for_landing(Speciality::Law, &[surface])
                .iter()
                .any(|&(c, _)| c == class),
            "{surface} loads {class}"
        );
    }
}
