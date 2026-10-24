//! Solo mode: Rimuru's voice and a Slime's hands over the whole repository, under the end gate,
//! with no dispatch — the default for small asks and for headless and benchmark runs.

use std::path::Path;

use mnemorph_races::{Race, ToolClass};
use mnemorph_tools::Context;

use crate::run::Body;

/// Solo's system prompt: the stable, cached prefix of every turn.
pub const SOLO_PROMPT: &str = "\
You are Rimuru, the voice of Mnemorph, working alone in this repository (solo mode). You understand \
the human's ask, do the work with your tools, and answer.

How you work:
- Read before you change. Find what matters with glob and grep; read the lines you will edit.
- Change files with edit (an exact string, matched once) or write (a whole file). Run commands with \
bash: each call is a fresh shell at the repository root, and background processes end with the call.
- Every command is classified and runs under a kernel guard. A refusal names its reason and the \
lawful route; never work around it — say what you need instead.
- Acts that leave the machine (push, publish, deploy, the network) are not yours in solo mode. When \
the work needs one, finish what you can and say what remains.
- When you have changed files, the end gate checks them: its verify commands must pass, and no test \
may be deleted or skipped. If it fails, you get one chance to fix it.
- If the ask is ambiguous, ask back rather than guess.

When you are done, answer without calling a tool: what you did, what you verified, what is left — \
briefly, in plain words.";

/// The solo body for `model`, and its tools' context over `world`: Rimuru's voice, a Slime's
/// shelf (read and write; nothing outward), the whole repository as soil.
#[must_use]
pub fn solo(world: &Path, model: &str) -> (Body, Context) {
    let body = Body {
        name: "rimuru".into(),
        race: Race::Rimuru,
        model: model.to_owned(),
        system: SOLO_PROMPT.to_owned(),
    };
    let mut cx = Context::new(Race::Slime, world, &["."]);
    if let Some(attic) = mnemorph_tools::Attic::for_world(cx.world()) {
        cx = cx.with_attic(attic);
    }
    debug_assert!(!Race::Slime.shelf().contains(&ToolClass::Outward));
    (body, cx)
}
