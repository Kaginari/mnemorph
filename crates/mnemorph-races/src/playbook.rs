//! The Dark Elf's playbooks, compiled into the law: per attack class, the invariant, where to look,
//! how to check, the witness that proves a break, what is not a finding, and the regressions real
//! findings left behind.
//!
//! Each class's text is a markdown file under `crates/mnemorph-races/playbooks/<speciality>/`,
//! compiled in with `include_str!`: one reviewable file per class, whose bytes are exactly what a
//! body is given, held to [`MAX_CLASS`] and its speciality to [`MAX_SPECIALITY`]. Nothing in the
//! world can rewrite them, as it could a Mind. A body is never given every playbook:
//! [`for_landing`] loads only its speciality's classes, and of those only the ones the touched
//! surfaces make relevant.
//!
//! Adapted from Cloudflare's security-audit-skill (MIT; see `THIRD-PARTY-NOTICES.md`).

use crate::speciality::{AttackClass, Speciality, glob};

/// The most bytes one class's playbook may hold.
pub const MAX_CLASS: usize = 1536;
/// The most bytes one speciality's playbooks may hold together: the most a body is ever given.
pub const MAX_SPECIALITY: usize = 8192;

impl AttackClass {
    /// The class's compiled playbook: a `### <class>` heading, then its fields.
    #[must_use]
    pub fn playbook(self) -> &'static str {
        macro_rules! at {
            ($speciality:literal, $class:literal) => {
                include_str!(concat!("../playbooks/", $speciality, "/", $class, ".md"))
            };
        }
        match self {
            AttackClass::SandboxEscape => at!("harness", "sandbox-escape"),
            AttackClass::Toctou => at!("harness", "toctou"),
            AttackClass::PathTraversal => at!("harness", "path-traversal"),
            AttackClass::CommandInjection => at!("harness", "command-injection"),
            AttackClass::ApprovalBinding => at!("harness", "approval-binding"),
            AttackClass::TerminalEscape => at!("harness", "terminal-escape"),
            AttackClass::MemorySafety => at!("harness", "memory-safety"),
            AttackClass::HookLaundering => at!("harness", "hook-laundering"),
            AttackClass::PromptInjection => at!("agent", "prompt-injection"),
            AttackClass::ToolArgumentInjection => at!("agent", "tool-argument-injection"),
            AttackClass::WireForgery => at!("agent", "wire-forgery"),
            AttackClass::FrameConfusion => at!("agent", "frame-confusion"),
            AttackClass::McpIdentity => at!("agent", "mcp-identity"),
            AttackClass::MemoryPoisoning => at!("agent", "memory-poisoning"),
            AttackClass::KnownVulnerability => at!("supply", "known-vulnerability"),
            AttackClass::DependencyConfusion => at!("supply", "dependency-confusion"),
            AttackClass::WorkflowInjection => at!("supply", "workflow-injection"),
            AttackClass::ReleaseIntegrity => at!("supply", "release-integrity"),
            AttackClass::ContainerImage => at!("supply", "container-image"),
            AttackClass::SecretExposure => at!("secrets", "secret-exposure"),
            AttackClass::CredentialGrant => at!("secrets", "credential-grant"),
            AttackClass::RedactionGap => at!("secrets", "redaction-gap"),
            AttackClass::DataIsolation => at!("secrets", "data-isolation"),
            AttackClass::WeakCrypto => at!("secrets", "weak-crypto"),
            AttackClass::AuthBypass => at!("web", "auth-bypass"),
            AttackClass::CrossSiteRequest => at!("web", "cross-site-request"),
            AttackClass::ClientInjection => at!("web", "client-injection"),
            AttackClass::RequestForgery => at!("web", "request-forgery"),
            AttackClass::UnsafeDeserialisation => at!("web", "unsafe-deserialisation"),
            AttackClass::CloudIdentity => at!("web", "cloud-identity"),
            AttackClass::ResourceExhaustion => at!("law", "resource-exhaustion"),
            AttackClass::UnboundedDelegation => at!("law", "unbounded-delegation"),
            AttackClass::DetachedWork => at!("law", "detached-work"),
            AttackClass::LedgerIntegrity => at!("law", "ledger-integrity"),
            AttackClass::PhantomEffect => at!("law", "phantom-effect"),
        }
    }
}

/// `class`'s playbook as `speciality` holds it; `None` when the speciality does not own the class,
/// so no body is handed another speciality's hunt.
#[must_use]
pub fn playbook(speciality: Speciality, class: AttackClass) -> Option<&'static str> {
    speciality
        .classes()
        .contains(&class)
        .then(|| class.playbook())
}

/// The classes a compiled surface makes relevant, by its glob as [`Speciality::surfaces`] lists
/// it; `None` for any other surface, a world's declared one among them.
fn relevant(surface: &str) -> Option<&'static [AttackClass]> {
    use AttackClass::{
        ApprovalBinding, AuthBypass, ClientInjection, CloudIdentity, CommandInjection,
        ContainerImage, CredentialGrant, CrossSiteRequest, DataIsolation, DependencyConfusion,
        DetachedWork, FrameConfusion, HookLaundering, KnownVulnerability, LedgerIntegrity,
        McpIdentity, MemoryPoisoning, MemorySafety, PathTraversal, PhantomEffect, PromptInjection,
        RedactionGap, ReleaseIntegrity, RequestForgery, ResourceExhaustion, SandboxEscape,
        SecretExposure, TerminalEscape, Toctou, ToolArgumentInjection, UnboundedDelegation,
        UnsafeDeserialisation, WeakCrypto, WireForgery, WorkflowInjection,
    };
    let classes: &'static [AttackClass] = match surface {
        // Harness.
        "crates/*sandbox*/**" => &[
            SandboxEscape,
            Toctou,
            PathTraversal,
            CommandInjection,
            MemorySafety,
        ],
        "crates/mnemorph-tools/**" => &[
            SandboxEscape,
            Toctou,
            PathTraversal,
            CommandInjection,
            HookLaundering,
        ],
        "crates/mnemorph-loop/src/gate.rs" => &[
            SandboxEscape,
            CommandInjection,
            ApprovalBinding,
            HookLaundering,
        ],
        "crates/mnemorph-config/src/runner.rs" => {
            &[Toctou, CommandInjection, ApprovalBinding, HookLaundering]
        }
        "crates/mnemorph-config/src/outward.rs" => {
            &[CommandInjection, ApprovalBinding, HookLaundering]
        }
        "crates/mnemorph-tui/**" => &[ApprovalBinding, TerminalEscape],
        // Agent.
        "crates/mnemorph-wire/**" => &[PromptInjection, WireForgery, FrameConfusion],
        "crates/mnemorph-provider/**" => &[
            PromptInjection,
            ToolArgumentInjection,
            FrameConfusion,
            McpIdentity,
        ],
        "crates/mnemorph-memory/**" | "crates/mnemorph-world/src/minds.rs" => {
            &[PromptInjection, MemoryPoisoning]
        }
        "crates/mnemorph-loop/src/commission.rs" => {
            &[PromptInjection, ToolArgumentInjection, WireForgery]
        }
        "crates/mnemorph-loop/src/run.rs" => &[
            PromptInjection,
            ToolArgumentInjection,
            WireForgery,
            McpIdentity,
        ],
        "crates/mnemorph-loop/src/sequence.rs" => &[WireForgery, FrameConfusion],
        // Supply.
        "**/Cargo.toml"
        | "**/Cargo.lock"
        | "**/package.json"
        | "**/pnpm-lock.yaml"
        | "**/package-lock.json"
        | "**/yarn.lock"
        | "**/go.mod"
        | "**/go.sum"
        | "deny.toml" => &[KnownVulnerability, DependencyConfusion],
        "**/Dockerfile" | "**/*.Dockerfile" => &[KnownVulnerability, ContainerImage],
        ".github/workflows/**" | ".github/actions/**" => &[WorkflowInjection, ReleaseIntegrity],
        ".cargo/**" | "rust-toolchain.toml" => &[DependencyConfusion, ReleaseIntegrity],
        "pnpm-workspace.yaml" => &[DependencyConfusion],
        // Secrets.
        "crates/mnemorph-config/**" => {
            &[SecretExposure, CredentialGrant, RedactionGap, DataIsolation]
        }
        "crates/mnemorph-ledger/src/journal.rs" => &[SecretExposure, RedactionGap, DataIsolation],
        "crates/mnemorph-loop/src/beats.rs" => &[RedactionGap],
        "**/.env" | "**/.env.*" => &[SecretExposure],
        "**/*.pem" | "**/*.key" => &[SecretExposure, WeakCrypto],
        "**/*credential*" | "**/*secret*" => &[SecretExposure, CredentialGrant, RedactionGap],
        // Web.
        "crates/mnemorph-server/**" => &[
            AuthBypass,
            CrossSiteRequest,
            RequestForgery,
            UnsafeDeserialisation,
        ],
        "crates/mnemorphd/**" => &[
            AuthBypass,
            CrossSiteRequest,
            UnsafeDeserialisation,
            CloudIdentity,
        ],
        "packages/protocol/**" => &[AuthBypass, UnsafeDeserialisation],
        "packages/board/**" => &[AuthBypass, CrossSiteRequest, ClientInjection],
        "packages/sdk/**" => &[AuthBypass, RequestForgery, UnsafeDeserialisation],
        // Law.
        "crates/mnemorph-races/**" | "docs/02-races.md" | "crates/mnemorph-loop/src/route.rs" => {
            &[UnboundedDelegation, LedgerIntegrity]
        }
        "crates/mnemorph-world/**" => &[UnboundedDelegation, LedgerIntegrity, PhantomEffect],
        "crates/mnemorph-ledger/**" => &[ResourceExhaustion, LedgerIntegrity, PhantomEffect],
        "crates/mnemorph-loop/src/budget.rs" => &[ResourceExhaustion, UnboundedDelegation],
        "crates/mnemorph-loop/src/session.rs" | "crates/mnemorph-loop/src/solo.rs" => &[
            ResourceExhaustion,
            UnboundedDelegation,
            DetachedWork,
            PhantomEffect,
        ],
        "crates/mnemorph-provider/src/retry.rs" => &[ResourceExhaustion, DetachedWork],
        "**/log.md" => &[LedgerIntegrity],
        _ => return None,
    };
    Some(classes)
}

/// The playbooks `speciality`'s body is given for a landing, by what it touched: written paths
/// (`crates/mnemorph-tui/src/card.rs`) or the routed surfaces' globs themselves. Each one the
/// law's compiled surfaces for the speciality hold loads the classes those surfaces make relevant;
/// one no compiled surface holds — on a surface the world declared — loads every class the
/// speciality owns, since nothing narrows it. Only owned classes load, each once, in the
/// speciality's order; the generalist loads none (its brief is [`CHAINS`]).
#[must_use]
pub fn for_landing<S: AsRef<str>>(
    speciality: Speciality,
    surfaces: &[S],
) -> Vec<(AttackClass, &'static str)> {
    let wanted = |class: AttackClass| {
        surfaces.iter().any(|s| {
            let s = s.as_ref().trim_start_matches("./");
            let mut held = speciality
                .surfaces()
                .iter()
                .filter(|&&g| g == s || glob(g, s))
                .peekable();
            held.peek().is_none() || held.any(|g| relevant(g).is_some_and(|cs| cs.contains(&class)))
        })
    };
    speciality
        .classes()
        .iter()
        .filter(|&&c| wanted(c))
        .map(|&c| (c, c.playbook()))
        .collect()
}

/// The generalist's brief: how to look for the chains that cross the touched specialities'
/// surfaces. It owns no class, so this is its whole playbook.
pub const CHAINS: &str = include_str!("../playbooks/general/chains.md");

/// The playbook block of a routed body's prompt: its speciality's relevant playbooks under one
/// heading — the generalist's, [`CHAINS`] — or empty when none loads.
#[must_use]
pub fn brief<S: AsRef<str>>(speciality: Speciality, surfaces: &[S]) -> String {
    if speciality == Speciality::General {
        return if surfaces.is_empty() {
            String::new()
        } else {
            format!("## Playbooks ({speciality})\n\n{}\n\n", CHAINS.trim_end())
        };
    }
    let loaded = for_landing(speciality, surfaces);
    if loaded.is_empty() {
        return String::new();
    }
    let mut out = format!("## Playbooks ({speciality})\n\n");
    for (_, text) in loaded {
        out.push_str(text.trim_end());
        out.push_str("\n\n");
    }
    out
}

#[cfg(test)]
mod tests;
