# Race cards — the art brief

A brief for an image model: one card per race and ascended form, plus a card back. The cards are
shown on the board, in the docs and in the decks.

## In plain words

The image model paints **the art and the frame only — no words, no numbers**. Image models
garble text, and the card text (name, duty, powers, score, bars) comes from the compiled race law
anyway, so the board writes it onto the empty plates. When the law changes, the cards change
with it, and the art never needs to be redone.

Generate in this order, so the set stays consistent:

1. **The Slime card first.** Iterate until the frame, style and lighting are right. It becomes
   the reference image for every other card (image-to-image or "style reference" in your model).
2. **The card back**, with the Slime card as reference.
3. The other base races, then the ascended forms — each ascended card with its base card as a
   second reference, so it reads as the same creature, evolved.

## The card spec

```
┌──────────────────────────────┐  1500 × 2100 px, portrait 5:7 (trading card 63 × 88 mm)
│ ◆   ┌──────────────────┐   ◆ │  ← tier gems: 1 for a base race, 2 for an ascended form
│     │   NAME BANNER    │     │  ← empty plate, 1100 × 150 px, top
│  ┌────────────────────────┐  │
│  │                        │  │
│  │                        │  │
│  │      ART WINDOW        │  │  ← 1300 × 1150 px; the character's head and shoulders
│  │                        │  │     sit in the centred upper 1000 × 1000 square
│  │                        │  │     (it is cropped 1:1 as the board/TUI portrait)
│  └──────────( ✦ )─────────┘  │  ← race sigil medallion, 220 px, overlapping the seam
│  ┌────────────────────────┐  │
│  │      DUTY PLATE        │  │  ← empty plate, 1300 × 520 px, smooth, low detail
│  └────────────────────────┘  │
│        ▭  ▭  ▭  ▭            │  ← four empty stat sockets
└──────────────────────────────┘  outer 60 px bleed, nothing important there
```

- Plates must be **empty, flat-ish and evenly lit**: the text is overlaid in both dark and light
  themes, so a plate is mid-dark glass or parchment with soft inner shadow, no ornament inside.
- PNG, sRGB. If the model offers transparency, keep the outside corners transparent (rounded
  corner radius 60 px).
- Every card in the set shares the same frame geometry; only material, colour and ornament
  change per race.

## Master style block

Paste this at the start of every card prompt.

> Premium fantasy trading card, portrait 5:7, full card visible, front view, flat-on, no
> perspective tilt. Original character design in polished anime key-visual style, crisp clean
> line art, cel shading with soft painterly gradients, rim light, high detail on the character,
> calm readable background. The card frame is ornate but clean, symmetric, made of the race's
> materials, with: an empty name banner at the top, a large art window, a round sigil medallion
> on the seam under the art window, an empty duty plate in the lower third, four empty stat
> sockets at the bottom, and tier gems in the top corners. All plates are blank and smooth —
> **absolutely no text, no letters, no numbers, no runes that look like writing, no watermark,
> no signature**. Character centred, head and shoulders in the upper middle of the art window,
> strong silhouette that still reads at 64 px. Consistent lighting from the upper left.
> Background inside the art window hints at the race's work, softly blurred.

**Negative prompt** (if the model takes one): text, letters, words, numbers, captions, logo,
watermark, signature, runes, writing on plates, extra fingers, deformed hands, cropped head,
off-centre character, tilted card, photo, 3D render, busy plates, lens flare over plates,
copyrighted characters.

## The cards

Each block is pasted after the master style block. Colours are given as names and hex so the
board's theme tokens can match them.

### Card back

> The back of the card set. Deep storm-indigo (#1E1B4B) field with a fine gold (#D4AF37) filigree
> border; in the centre a large circular emblem: a coiled storm dragon around a glowing
> droplet-shaped slime crown, surrounded by a ring of six small sigils (crown-drop, leaf, axe-gate,
> droplet, torii, crescent). Symmetric, no text.

### 🐉 Veldora — the human (special card, gold edge)

> Frame of storm-gold (#D4AF37) and indigo lightning, a third tier gem in the top centre. A vast
> storm dragon, wise and amused, coiled around the art window; only its head, one golden eye and
> crackling indigo storm clouds are visible, looking down at a small glowing slime on an open
> palm of cloud. Sigil: a dragon's slit eye inside a storm ring. Mood: the one whose word is law.

### 👑 Rimuru — the throne

> Frame of white gold and azure crystal (#3FA9F5). A golden, softly glowing slime sovereign
> (#F2C94C), round and content, eyes closed in a gentle smile — same character as a small round
> golden slime with blush marks — floating above a throne of light, wearing a delicate azure
> crystal crown. Behind it, faint glowing threads fan downward to many tiny distant figures,
> suggesting it orders a whole realm. Sigil: a crown resting on a droplet. Mood: calm authority,
> the only voice that speaks to the human.

### 🌿 Elf — the lane's planner and voice

> Frame of living wood and emerald (#2FA36B) with gold leaf. A young elf woman with long straight
> blonde hair, green eyes, a golden laurel circlet, pointed ears, green and cream robes. She holds
> a translucent map of glowing paths that branch into several lanes, one hand raised as if
> speaking. Background: a forest canopy with soft light. Sigil: a leaf crossed by a compass
> needle. Mood: sees across domains, plans, speaks for the lane.

### 🪓 Orc — the domain and its gate

> Frame of hammered iron with amber (#C9822B) inlays and rivets. A broad green-skinned orc with
> big lower tusks, a small ring earring, a stern but fair expression, standing before a massive
> closed iron gate, one hand resting on a great axe planted in the ground, the other holding up a
> glowing amber seal stamp. Background: a fortified stone domain at dusk. Sigil: an axe crossed
> with a gate. Mood: nothing passes without its verdict.

### 💧 Slime — the ground truth of a zone

> Frame of polished teal glass (#19C3B0) and silver, with small bubble motifs. A round, glossy
> aqua-blue slime (#4FC3F7) with big red eyes, blush marks and a small happy mouth, sitting on
> a neat glowing grid tile in a small walled garden — its zone — carefully placing a tiny
> luminous cube into place. A tiny scroll (its zone document, blank) floats next to it. Sigil: a
> droplet holding a small cube. Mood: small, exact, the only one who touches the code.

### 👹 Kijin — the hands that reach outside

> Frame of black lacquer and crimson (#D7263D) with gold torii motifs. A young oni woman with a
> single ivory horn, long dark indigo hair with red streaks, red eyes, red marks on her cheeks, a
> small fang, wearing a dark kimono-style uniform with a crimson sash. She stands at an open torii
> gate at the edge of a village; through it, beyond, a wide outside world with distant ships,
> towers and signal fires. She carries a sealed scroll-case and a lantern. Sigil: a torii gate
> with an outward arrow. Mood: disciplined, loyal, the only one who leaves the village — and only
> with the lord's seal.

Optional **post variants** (same character design, same frame, different prop and background):

| Post | Prop and background |
|---|---|
| `kijin-ci` | a wooden training sword, a dojo with rows of practice targets lit green and red |
| `kijin-release` | a smith's hammer over an anvil, a freshly forged blade glowing, crates being sealed |
| `kijin-deploy` | a commander's war fan, a map table with banners planted on distant castles |
| `kijin-forge` | a brush and a stack of sealed letters, a shrine archive with hanging record scrolls |
| `kijin-scout` | a short blade and a hooded cloak, a rooftop at night overlooking distant lights |

### 🌑 Dark Elf — the adversary

> Frame of obsidian and violet (#7B4FD6) with thin silver edges, slightly asymmetric thorn
> ornaments. A dark elf woman with grey-violet skin, long silver hair, grey eyes, gold drop
> earrings, a knowing half-smile with a small fang, in a high-collared black and gold uniform.
> She holds a slim silver lens up to her eye, through which a hairline crack in a stone wall
> glows; her other hand pins a glowing violet seal onto the crack. Background: moonlit ramparts
> of a fortress, seen from outside. Sigil: a crescent moon pierced by a needle. Mood: hostile to
> the work, honest about the proof.

### ✨ High Elf — ascended Elf

> Same elf as the Elf card, evolved: hair now with teal-white luminous strands, an emerald and
> white-gold tiara with a central gem, green and white high-collared robes, a soft white-green aura
> and floating motes of light. The glowing map in her hands now connects to several other forests'
> maps at the edges. Frame: the Elf frame in white gold (#F5E6B8) with emerald, two tier gems,
> faint halo behind the art window. Sigil: the Elf sigil inside a radiant ring.

### ⚔️ High Orc — ascended Orc

> Same orc as the Orc card, evolved: long brown hair, a horned bone-and-iron helm, heavier tusks,
> scarred and proud, a fur-trimmed mantle over iron armour. The great gate behind him now stands
> open on a road he guards; his amber seal stamp glows molten gold (#E8A33D). Frame: the Orc frame
> in blackened iron with molten-gold seams, two tier gems. Sigil: the Orc sigil under a small
> crown.

### 💎 Named Slime — ascended Slime

> Same slime as the Slime card, evolved: its body now faceted like a clear aquamarine crystal
> (#5EEAD4) with light refracting inside, a tiny glowing name-mark on its forehead (a simple star
> shape, not a letter), the same red eyes. It tends a larger, more intricate garden of glowing
> tiles. Frame: the Slime frame in crystal and platinum, two tier gems. Sigil: the Slime sigil cut
> as a gemstone.

### 🔥 Oni — ascended Kijin

> Same oni woman as the Kijin card, evolved: the single horn now longer with a faint flame-gold
> glow, hair streaks burning crimson-orange (#FF6B35), embers drifting around her, a formal
> crimson and gold haori over the uniform, a ribbon of folded sealed charters (the human's
> standing permissions, blank) tied at her hip. The torii behind her is now lit and wreathed in
> calm flame; the road beyond is lit by lanterns she has placed. Frame: lacquer and gold with flame
> filigree, two tier gems. Sigil: the torii sigil inside a flame ring.

### 🌘 Arch Dark Elf — ascended Dark Elf

> Same dark elf as the Dark Elf card, evolved: silver hair now with violet-black tips, a circlet
> of dark silver with a crescent, an eclipse (#2E1065 with a silver corona) behind her head, a
> long coat with violet lining. Several violet seals hover around her, each pinned on a different
> faint crack in the fortress wall behind. Frame: obsidian and silver with eclipse motifs, two
> tier gems. Sigil: the Dark Elf sigil over an eclipse.

## What I check when the results come back

Send them in whatever form the model gives; I review each against:

1. **No text anywhere** — including fake runes on plates.
2. **Frame geometry** — banner, art window, medallion, duty plate and sockets at the same place
   on every card (I overlay them and measure).
3. **Plates usable** — text overlaid in both themes stays at WCAG AA contrast.
4. **1:1 crop** — the head-and-shoulders square works as a 64 px and 256 px portrait.
5. **Continuity** — each creature is recognisable from its isekai portrait, and each ascended
   form from its base card.
6. **Duty readable** — the prop and background say the race's job without a caption.
7. **Set harmony** — the race colours are distinct for colour-blind viewers (I run the CVD check).

Cards that pass go to `assets/cards/<race>.png` with the crops in `assets/portraits/`; the board
renders the text plates from the race law.
