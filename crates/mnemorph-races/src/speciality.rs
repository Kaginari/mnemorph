//! Dark Elf specialities: what each one hunts (its attack classes) and where (its surfaces), and
//! the routing of a landing's written paths to the specialities they touch.
//!
//! A speciality is read from the creature's name by exact match, as a Kijin's post is. Each
//! attack class has exactly one owning speciality, so two specialities never hunt the same class
//! on the same paths. The surfaces here are compiled; a world adds its own in its Dark Elves'
//! docs, and [`route_with`] takes them.

use std::fmt;

/// A Dark Elf's speciality: six hunters and the generalist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Speciality {
    /// The sandbox, the shell and file tools, the classifier, the gate, the act runner, terminal
    /// rendering.
    Harness,
    /// The wire, commissions and prompts, memory and Minds, providers, MCP.
    Agent,
    /// Lockfiles and manifests, CI workflows, release config, Dockerfiles.
    Supply,
    /// Configuration, credentials, redaction, journals, history.
    Secrets,
    /// Servers, the protocol package, the board.
    Web,
    /// `log.md`, gate verdicts, envelopes, caps, retries, background work, the race law.
    Law,
    /// The plain `dark-elf`: the threat model at each Orc's birth and the attack chains that
    /// cross surfaces. It owns no class and no surface.
    General,
}

impl Speciality {
    /// Every speciality, the generalist last.
    pub const ALL: [Speciality; 7] = [
        Speciality::Harness,
        Speciality::Agent,
        Speciality::Supply,
        Speciality::Secrets,
        Speciality::Web,
        Speciality::Law,
        Speciality::General,
    ];

    /// The six that own surfaces and classes.
    pub const HUNTERS: [Speciality; 6] = [
        Speciality::Harness,
        Speciality::Agent,
        Speciality::Supply,
        Speciality::Secrets,
        Speciality::Web,
        Speciality::Law,
    ];

    /// The creature name of the speciality: `dark-elf-harness`, …, and `dark-elf`.
    #[must_use]
    pub fn creature(self) -> &'static str {
        match self {
            Speciality::Harness => "dark-elf-harness",
            Speciality::Agent => "dark-elf-agent",
            Speciality::Supply => "dark-elf-supply",
            Speciality::Secrets => "dark-elf-secrets",
            Speciality::Web => "dark-elf-web",
            Speciality::Law => "dark-elf-law",
            Speciality::General => "dark-elf",
        }
    }

    /// The speciality a creature holds, by its exact name: `dark-elf-supply` holds supply,
    /// `dark-elf` the generalist's. Any other name, `dark-elf-agent-2` included, holds none.
    #[must_use]
    pub fn of(creature: &str) -> Option<Speciality> {
        Speciality::ALL
            .into_iter()
            .find(|s| s.creature() == creature)
    }

    /// The attack classes this speciality owns. The generalist owns none.
    #[must_use]
    pub fn classes(self) -> &'static [AttackClass] {
        use AttackClass::{
            ApprovalBinding, AuthBypass, ClientInjection, CloudIdentity, CommandInjection,
            ContainerImage, CredentialGrant, CrossSiteRequest, DataIsolation, DependencyConfusion,
            DetachedWork, FrameConfusion, HookLaundering, KnownVulnerability, LedgerIntegrity,
            McpIdentity, MemoryPoisoning, MemorySafety, PathTraversal, PhantomEffect,
            PromptInjection, RedactionGap, ReleaseIntegrity, RequestForgery, ResourceExhaustion,
            SandboxEscape, SecretExposure, TerminalEscape, Toctou, ToolArgumentInjection,
            UnboundedDelegation, UnsafeDeserialisation, WeakCrypto, WireForgery, WorkflowInjection,
        };
        match self {
            Speciality::Harness => &[
                SandboxEscape,
                Toctou,
                PathTraversal,
                CommandInjection,
                ApprovalBinding,
                TerminalEscape,
                MemorySafety,
                HookLaundering,
            ],
            Speciality::Agent => &[
                PromptInjection,
                ToolArgumentInjection,
                WireForgery,
                FrameConfusion,
                McpIdentity,
                MemoryPoisoning,
            ],
            Speciality::Supply => &[
                KnownVulnerability,
                DependencyConfusion,
                WorkflowInjection,
                ReleaseIntegrity,
                ContainerImage,
            ],
            Speciality::Secrets => &[
                SecretExposure,
                CredentialGrant,
                RedactionGap,
                DataIsolation,
                WeakCrypto,
            ],
            Speciality::Web => &[
                AuthBypass,
                CrossSiteRequest,
                ClientInjection,
                RequestForgery,
                UnsafeDeserialisation,
                CloudIdentity,
            ],
            Speciality::Law => &[
                ResourceExhaustion,
                UnboundedDelegation,
                DetachedWork,
                LedgerIntegrity,
                PhantomEffect,
            ],
            Speciality::General => &[],
        }
    }

    /// The compiled surfaces this speciality is routed, as path globs relative to the world:
    /// `*` stays inside one path segment, `**` spans any number of them. The generalist has
    /// none: it is routed by [`route`]'s cross-surface rule alone.
    #[must_use]
    pub fn surfaces(self) -> &'static [&'static str] {
        match self {
            Speciality::Harness => &[
                "crates/*sandbox*/**",
                "crates/mnemorph-tools/**",
                "crates/mnemorph-loop/src/gate.rs",
                "crates/mnemorph-config/src/runner.rs",
                "crates/mnemorph-config/src/outward.rs",
                "crates/mnemorph-tui/**",
            ],
            Speciality::Agent => &[
                "crates/mnemorph-wire/**",
                "crates/mnemorph-provider/**",
                "crates/mnemorph-memory/**",
                "crates/mnemorph-loop/src/commission.rs",
                "crates/mnemorph-loop/src/run.rs",
                "crates/mnemorph-loop/src/sequence.rs",
                "crates/mnemorph-world/src/minds.rs",
            ],
            Speciality::Supply => &[
                "**/Cargo.toml",
                "**/Cargo.lock",
                "**/package.json",
                "**/pnpm-lock.yaml",
                "**/package-lock.json",
                "**/yarn.lock",
                "**/go.mod",
                "**/go.sum",
                "**/Dockerfile",
                "**/*.Dockerfile",
                ".github/workflows/**",
                ".github/actions/**",
                ".cargo/**",
                "deny.toml",
                "rust-toolchain.toml",
                "pnpm-workspace.yaml",
            ],
            Speciality::Secrets => &[
                "crates/mnemorph-config/**",
                "crates/mnemorph-ledger/src/journal.rs",
                "crates/mnemorph-loop/src/beats.rs",
                "**/.env",
                "**/.env.*",
                "**/*.pem",
                "**/*.key",
                "**/*credential*",
                "**/*secret*",
            ],
            Speciality::Web => &[
                "crates/mnemorph-server/**",
                "crates/mnemorphd/**",
                "packages/protocol/**",
                "packages/board/**",
                "packages/sdk/**",
            ],
            Speciality::Law => &[
                "crates/mnemorph-races/**",
                "crates/mnemorph-world/**",
                "crates/mnemorph-ledger/**",
                "crates/mnemorph-loop/src/budget.rs",
                "crates/mnemorph-loop/src/route.rs",
                "crates/mnemorph-loop/src/session.rs",
                "crates/mnemorph-loop/src/solo.rs",
                "crates/mnemorph-provider/src/retry.rs",
                "docs/02-races.md",
                "**/log.md",
            ],
            Speciality::General => &[],
        }
    }

    /// Whether `path` (relative to the world, `/`-separated) lies on one of this speciality's
    /// compiled surfaces.
    #[must_use]
    pub fn touches(self, path: &str) -> bool {
        self.surfaces().iter().any(|g| glob(g, path))
    }
}

impl fmt::Display for Speciality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.creature())
    }
}

/// An attack class: one kind of break a speciality's playbook hunts. Each has exactly one owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AttackClass {
    /// A sandboxed command reaching what its grant does not give it.
    SandboxEscape,
    /// A check and the use it guards seeing different files: a symlink or rename swapped between.
    Toctou,
    /// A path climbing or resolving out of the ground it was confined to.
    PathTraversal,
    /// Untrusted text reaching a process's command line or a shell unquoted.
    CommandInjection,
    /// The act that runs differing from the card the human approved.
    ApprovalBinding,
    /// Escape sequences in model or tool text driving the terminal.
    TerminalEscape,
    /// Memory unsafety behind `unsafe` or FFI.
    MemorySafety,
    /// Instructions smuggled in through repository text or tool output.
    PromptInjection,
    /// Model-chosen tool arguments that change what the tool does.
    ToolArgumentInjection,
    /// A wire line (`@F`, `@S`) forged through prose or tool output.
    WireForgery,
    /// Framing a stream or protocol reads two ways: truncation, splitting, smuggling.
    FrameConfusion,
    /// An MCP server or tool passing as another, or bypassing the conduit.
    McpIdentity,
    /// A note, a Mind or the log written to steer later bodies.
    MemoryPoisoning,
    /// A dependency with a known advisory.
    KnownVulnerability,
    /// A dependency resolved from a source or name it was not meant to be.
    DependencyConfusion,
    /// Untrusted event data reaching a CI step's script, or a workflow's token over-granted.
    WorkflowInjection,
    /// Git config, hooks or an environment prefix laundering an act past its classifier.
    HookLaundering,
    /// A release artifact not bound to the source and build it claims.
    ReleaseIntegrity,
    /// A container image unpinned, over-privileged or carrying what it should not.
    ContainerImage,
    /// A secret in the tree or its history.
    SecretExposure,
    /// A credential read through a grant meant for something else.
    CredentialGrant,
    /// A secret written past redaction into a journal, a log or a transcript.
    RedactionGap,
    /// One world, session or user reading another's data.
    DataIsolation,
    /// Cryptography misused: a weak primitive, a reused nonce, a comparison in variable time.
    WeakCrypto,
    /// A request served without the authentication or authorisation it needs.
    AuthBypass,
    /// A cross-site request the server cannot tell from its own client's.
    CrossSiteRequest,
    /// Script or markup injected into a client that renders it.
    ClientInjection,
    /// The server made to fetch what an attacker names.
    RequestForgery,
    /// Untrusted bytes deserialised into more than data.
    UnsafeDeserialisation,
    /// A cloud identity or policy granting more than its holder needs.
    CloudIdentity,
    /// Work without a bound: tokens, time, memory, disk, retries.
    ResourceExhaustion,
    /// Delegation that never ends: commissions or loops with no cap.
    UnboundedDelegation,
    /// Work that outlives the interrupt meant to stop it.
    DetachedWork,
    /// `log.md`, a gate verdict or the ledger altered or forged after the fact.
    LedgerIntegrity,
    /// Pre-existing state read as an act's observed effect.
    PhantomEffect,
}

impl AttackClass {
    /// Every class, in the order the specialities own them.
    pub const ALL: [AttackClass; 35] = [
        AttackClass::SandboxEscape,
        AttackClass::Toctou,
        AttackClass::PathTraversal,
        AttackClass::CommandInjection,
        AttackClass::ApprovalBinding,
        AttackClass::TerminalEscape,
        AttackClass::MemorySafety,
        AttackClass::HookLaundering,
        AttackClass::PromptInjection,
        AttackClass::ToolArgumentInjection,
        AttackClass::WireForgery,
        AttackClass::FrameConfusion,
        AttackClass::McpIdentity,
        AttackClass::MemoryPoisoning,
        AttackClass::KnownVulnerability,
        AttackClass::DependencyConfusion,
        AttackClass::WorkflowInjection,
        AttackClass::ReleaseIntegrity,
        AttackClass::ContainerImage,
        AttackClass::SecretExposure,
        AttackClass::CredentialGrant,
        AttackClass::RedactionGap,
        AttackClass::DataIsolation,
        AttackClass::WeakCrypto,
        AttackClass::AuthBypass,
        AttackClass::CrossSiteRequest,
        AttackClass::ClientInjection,
        AttackClass::RequestForgery,
        AttackClass::UnsafeDeserialisation,
        AttackClass::CloudIdentity,
        AttackClass::ResourceExhaustion,
        AttackClass::UnboundedDelegation,
        AttackClass::DetachedWork,
        AttackClass::LedgerIntegrity,
        AttackClass::PhantomEffect,
    ];

    /// The class's name, as a playbook and a finding's record cite it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            AttackClass::SandboxEscape => "sandbox-escape",
            AttackClass::Toctou => "toctou",
            AttackClass::PathTraversal => "path-traversal",
            AttackClass::CommandInjection => "command-injection",
            AttackClass::ApprovalBinding => "approval-binding",
            AttackClass::TerminalEscape => "terminal-escape",
            AttackClass::MemorySafety => "memory-safety",
            AttackClass::PromptInjection => "prompt-injection",
            AttackClass::ToolArgumentInjection => "tool-argument-injection",
            AttackClass::WireForgery => "wire-forgery",
            AttackClass::FrameConfusion => "frame-confusion",
            AttackClass::McpIdentity => "mcp-identity",
            AttackClass::MemoryPoisoning => "memory-poisoning",
            AttackClass::KnownVulnerability => "known-vulnerability",
            AttackClass::DependencyConfusion => "dependency-confusion",
            AttackClass::WorkflowInjection => "workflow-injection",
            AttackClass::HookLaundering => "hook-laundering",
            AttackClass::ReleaseIntegrity => "release-integrity",
            AttackClass::ContainerImage => "container-image",
            AttackClass::SecretExposure => "secret-exposure",
            AttackClass::CredentialGrant => "credential-grant",
            AttackClass::RedactionGap => "redaction-gap",
            AttackClass::DataIsolation => "data-isolation",
            AttackClass::WeakCrypto => "weak-crypto",
            AttackClass::AuthBypass => "auth-bypass",
            AttackClass::CrossSiteRequest => "cross-site-request",
            AttackClass::ClientInjection => "client-injection",
            AttackClass::RequestForgery => "request-forgery",
            AttackClass::UnsafeDeserialisation => "unsafe-deserialisation",
            AttackClass::CloudIdentity => "cloud-identity",
            AttackClass::ResourceExhaustion => "resource-exhaustion",
            AttackClass::UnboundedDelegation => "unbounded-delegation",
            AttackClass::DetachedWork => "detached-work",
            AttackClass::LedgerIntegrity => "ledger-integrity",
            AttackClass::PhantomEffect => "phantom-effect",
        }
    }

    /// The specialities whose playbook holds this class. The law holds it to exactly one.
    #[must_use]
    pub fn owners(self) -> Vec<Speciality> {
        Speciality::ALL
            .into_iter()
            .filter(|s| s.classes().contains(&self))
            .collect()
    }
}

impl fmt::Display for AttackClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The specialities a landing is routed to, by its written paths and the compiled surfaces.
/// See [`route_with`].
#[must_use]
pub fn route<P: AsRef<str>>(paths: &[P]) -> Vec<Speciality> {
    route_with(paths, |_, _| false)
}

/// The specialities a landing is routed to: every hunter whose surfaces — compiled, or
/// `declared` by the world for it — any written path touches, in [`Speciality::HUNTERS`] order;
/// and the generalist last when two or more hunters are, for the chains that cross surfaces, or
/// when a path touches the surfaces the world declares for it, its catch-all. Empty when no path
/// touches a surface.
#[must_use]
pub fn route_with<P: AsRef<str>>(
    paths: &[P],
    declared: impl Fn(Speciality, &str) -> bool,
) -> Vec<Speciality> {
    let touched = |s: Speciality| {
        paths.iter().any(|p| {
            let p = p.as_ref().trim_start_matches("./");
            s.touches(p) || declared(s, p)
        })
    };
    let mut routed: Vec<Speciality> = Speciality::HUNTERS
        .into_iter()
        .filter(|&s| touched(s))
        .collect();
    if routed.len() >= 2 || touched(Speciality::General) {
        routed.push(Speciality::General);
    }
    routed
}

/// Whether the surface glob `pattern` matches the whole of `path`, segment by segment: `**`
/// matches any number of segments, `*` any run of characters inside one. It is how a written path
/// is placed on a compiled surface, and how a coverage ledger's evidence is held to one.
#[must_use]
pub fn glob(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    segments(&pattern, &path)
}

fn segments(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| segments(rest, &path[skip..])),
        Some((first, rest)) => path
            .split_first()
            .is_some_and(|(seg, tail)| segment(first, seg) && segments(rest, tail)),
    }
}

/// Whether one segment matches, `*` standing for any run of characters.
fn segment(pattern: &str, seg: &str) -> bool {
    let mut parts = pattern.split('*');
    let head = parts.next().unwrap_or_default();
    let Some(mut rest) = seg.strip_prefix(head) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn of_is_exact() {
        for s in Speciality::ALL {
            assert_eq!(Speciality::of(s.creature()), Some(s));
            assert_eq!(s.to_string(), s.creature());
        }
        assert_eq!(Speciality::of("dark-elf"), Some(Speciality::General));
        for name in [
            "dark-elf-agent-2",
            "dark-elf-",
            "dark-elf-HARNESS",
            "arch-dark-elf",
            "dark-elves",
            "kijin-forge",
            "",
        ] {
            assert_eq!(Speciality::of(name), None, "{name}");
        }
    }

    /// The class table, listed so a change to it is deliberate.
    #[test]
    fn every_class_is_owned_once() {
        let table: Vec<(&str, Vec<&str>)> = Speciality::ALL
            .into_iter()
            .map(|s| (s.creature(), s.classes().iter().map(|c| c.name()).collect()))
            .collect();
        assert_eq!(
            table,
            [
                (
                    "dark-elf-harness",
                    vec![
                        "sandbox-escape",
                        "toctou",
                        "path-traversal",
                        "command-injection",
                        "approval-binding",
                        "terminal-escape",
                        "memory-safety",
                        "hook-laundering",
                    ]
                ),
                (
                    "dark-elf-agent",
                    vec![
                        "prompt-injection",
                        "tool-argument-injection",
                        "wire-forgery",
                        "frame-confusion",
                        "mcp-identity",
                        "memory-poisoning",
                    ]
                ),
                (
                    "dark-elf-supply",
                    vec![
                        "known-vulnerability",
                        "dependency-confusion",
                        "workflow-injection",
                        "release-integrity",
                        "container-image",
                    ]
                ),
                (
                    "dark-elf-secrets",
                    vec![
                        "secret-exposure",
                        "credential-grant",
                        "redaction-gap",
                        "data-isolation",
                        "weak-crypto",
                    ]
                ),
                (
                    "dark-elf-web",
                    vec![
                        "auth-bypass",
                        "cross-site-request",
                        "client-injection",
                        "request-forgery",
                        "unsafe-deserialisation",
                        "cloud-identity",
                    ]
                ),
                (
                    "dark-elf-law",
                    vec![
                        "resource-exhaustion",
                        "unbounded-delegation",
                        "detached-work",
                        "ledger-integrity",
                        "phantom-effect",
                    ]
                ),
                ("dark-elf", vec![]),
            ]
        );
        for c in AttackClass::ALL {
            assert_eq!(c.owners().len(), 1, "{c} has one owner");
            assert_eq!(c.to_string(), c.name());
        }
        let owned: usize = Speciality::ALL.map(|s| s.classes().len()).iter().sum();
        assert_eq!(owned, AttackClass::ALL.len(), "every owned class is listed");
        assert_eq!(Speciality::General.surfaces(), [] as [&str; 0]);
        for s in Speciality::HUNTERS {
            assert!(!s.surfaces().is_empty(), "{s}");
        }
    }

    #[test]
    fn route_one_surface() {
        assert_eq!(
            route(&["crates/mnemorph-sandbox/src/linux.rs"]),
            [Speciality::Harness]
        );
        assert_eq!(route(&["./Cargo.lock"]), [Speciality::Supply]);
        assert_eq!(
            route(&[
                "packages/board/src/app.ts",
                "crates/mnemorph-server/src/lib.rs"
            ]),
            [Speciality::Web]
        );
        assert_eq!(route(&[".mnemorph/log.md"]), [Speciality::Law]);
        assert_eq!(route(&["src/.env.local"]), [Speciality::Secrets]);
        assert_eq!(
            route(&["crates/mnemorph-wire/src/report.rs"]),
            [Speciality::Agent]
        );
    }

    #[test]
    fn route_two_surfaces_adds_the_generalist() {
        assert_eq!(
            route(&[
                "crates/mnemorph-tools/src/shell.rs",
                ".github/workflows/ci.yml",
            ]),
            [Speciality::Harness, Speciality::Supply, Speciality::General]
        );
        // One path on two surfaces is two hunters too.
        assert_eq!(
            route(&["crates/mnemorph-races/Cargo.toml"]),
            [Speciality::Supply, Speciality::Law, Speciality::General]
        );
    }

    #[test]
    fn route_none() {
        assert_eq!(route::<&str>(&[]), Vec::<Speciality>::new());
        assert_eq!(
            route(&["README.md", "docs/03-loop.md", "site/index.html"]),
            Vec::<Speciality>::new()
        );
    }

    /// The law's own ground — the roster, ownership, the ledger, the loop's routing — routes the
    /// law, so a landing that rewrites who is who is never unrouted.
    #[test]
    fn route_the_law_ground() {
        for path in [
            "crates/mnemorph-world/src/roster.rs",
            "crates/mnemorph-world/src/own.rs",
            "crates/mnemorph-ledger/src/fold.rs",
            "crates/mnemorph-loop/src/route.rs",
            "crates/mnemorph-loop/src/session.rs",
            "crates/mnemorph-loop/src/solo.rs",
        ] {
            assert!(route(&[path]).contains(&Speciality::Law), "{path}");
        }
    }

    #[test]
    fn the_generalist_is_routed_its_declared_surfaces() {
        let declared = |s: Speciality, p: &str| s == Speciality::General && p.starts_with("x/");
        assert_eq!(route_with(&["x/y.rs"], declared), [Speciality::General]);
        assert_eq!(
            route_with(&["x/y.rs", "Cargo.lock"], declared),
            [Speciality::Supply, Speciality::General]
        );
        assert_eq!(route_with(&["z/y.rs"], declared), Vec::<Speciality>::new());
    }

    #[test]
    fn route_with_declared_surfaces() {
        let declared = |s: Speciality, p: &str| s == Speciality::Web && p.starts_with("api/");
        assert_eq!(route_with(&["api/auth.rs"], declared), [Speciality::Web]);
        assert_eq!(
            route_with(&["api/auth.rs", "go.sum"], declared),
            [Speciality::Supply, Speciality::Web, Speciality::General]
        );
    }

    #[test]
    fn globs() {
        assert!(glob("**/Cargo.toml", "Cargo.toml"));
        assert!(glob("**/Cargo.toml", "a/b/Cargo.toml"));
        assert!(!glob("**/Cargo.toml", "a/Cargo.toml.bak"));
        assert!(glob(
            "crates/*sandbox*/**",
            "crates/mnemorph-sandbox/src/bin/x.rs"
        ));
        assert!(!glob(
            "crates/*sandbox*/**",
            "crates/mnemorph-tools/sandbox.rs"
        ));
        assert!(glob("**/*.Dockerfile", "ops/web.Dockerfile"));
        assert!(!glob("**/*.Dockerfile", "ops/Dockerfile"));
        assert!(glob("**/*secret*", "k8s/my-secrets.yaml"));
        assert!(glob("**/.env.*", ".env.prod"));
        assert!(!glob("**/.env.*", ".env"));
        assert!(glob("a*b*c", "aXbYc"));
        assert!(!glob("a*b*c", "aXc"));
        assert!(!glob("ab*b", "ab"));
        assert!(!glob("deny.toml", "x/deny.toml"));
        assert!(!glob("docs/02-races.md", "docs"));
    }
}
