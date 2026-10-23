# Race icons — Leonardo.ai prompts

The icons are the small portraits used in the terminal, the board's net and the docs: one per race
and ascended form. They are the same characters as the [race cards](race-cards-prompt.md),
head and shoulders on a round badge, readable at 32 px.

## The set

Generated from these prompts and cropped to `assets/portraits/` (256, 64 and 32 px, transparent).

| <img src="../assets/portraits/slime@64.png" width="64" alt="Slime"> | <img src="../assets/portraits/rimuru@64.png" width="64" alt="Rimuru"> | <img src="../assets/portraits/elf@64.png" width="64" alt="Elf"> | <img src="../assets/portraits/orc@64.png" width="64" alt="Orc"> | <img src="../assets/portraits/kijin@64.png" width="64" alt="Kijin"> | <img src="../assets/portraits/dark-elf@64.png" width="64" alt="Dark Elf"> |
|:---:|:---:|:---:|:---:|:---:|:---:|
| Slime | Rimuru | Elf | Orc | Kijin | Dark Elf |

| <img src="../assets/portraits/veldora@64.png" width="64" alt="Veldora"> | <img src="../assets/portraits/high-elf@64.png" width="64" alt="High Elf"> | <img src="../assets/portraits/high-orc@64.png" width="64" alt="High Orc"> | <img src="../assets/portraits/named-slime@64.png" width="64" alt="Named Slime"> | <img src="../assets/portraits/oni@64.png" width="64" alt="Oni"> | <img src="../assets/portraits/arch-dark-elf@64.png" width="64" alt="Arch Dark Elf"> |
|:---:|:---:|:---:|:---:|:---:|:---:|
| Veldora | High Elf | High Orc | Named Slime | Oni | Arch Dark Elf |

## Settings

- **Aspect ratio** 1:1, the largest square the model offers (1024 × 1024 or more).
- **Model**: a stylised/anime-capable one; keep the same model for the whole set.
- **Consistency**: generate the Slime first and iterate until it is right. Then, for every other
  icon, add it under Image Guidance as a **Style Reference**. For an ascended form, also add its
  base icon as a **Character Reference** (or a second image reference) so it reads as the same
  creature, evolved.
- **Background**: if the model offers a transparency option, turn it on; otherwise keep the plain
  white background the prompts ask for and remove it afterwards.
- **Negative prompt** (paste it when the model shows the field):

```
text, letters, words, numbers, watermark, signature, logo, frame cut off, cropped head, off-centre, multiple characters, full body, busy background, extra fingers, deformed hands, blurry, photo, realistic 3D render, copyrighted character
```

## Prompts

Each one is complete — paste it as it is, **one prompt per generation**. Leonardo caps the
prompt at 2,000 characters; every prompt here is 481–634 characters. The heading, the reference
note and the negative prompt are not part of the prompt — the negative prompt has its own field.

### 💧 Slime — first, the style reference for all

```
Game icon, round badge, centred, front view. A cute round glossy aqua-blue slime with big red eyes, pink blush marks and a small happy mouth, soft highlights on its jelly surface. Badge: a thick teal glass ring (#19C3B0) with a thin silver rim and small bubble details, a soft teal glow inside. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### 👑 Rimuru — the throne

```
Game icon, round badge, centred, front view. A softly glowing golden slime sovereign (#F2C94C), round and content, eyes closed in a gentle smile, pink blush marks, wearing a small delicate azure crystal crown. Badge: a white-gold ring set with azure crystals (#3FA9F5), a calm golden glow behind the slime. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### 🌿 Elf — the planner and voice

```
Game icon, round badge, centred, head and shoulders, front view. A young elf woman with long straight blonde hair, bright green eyes, pointed ears, a golden laurel circlet, green and cream high-collared robe, a calm confident smile. Badge: a ring of living wood and emerald (#2FA36B) with small gold leaves, soft forest-green light behind her. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### 🪓 Orc — the domain and its gate

```
Game icon, round badge, centred, head and shoulders, front view. A broad green-skinned orc with big lower tusks, a small ring earring, heavy brow, stern but fair expression, iron shoulder plates. Badge: a hammered iron ring with rivets and amber inlays (#C9822B), the faint shape of a closed iron gate behind him. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### 👹 Kijin — the hands that reach outside

```
Game icon, round badge, centred, head and shoulders, front view. A young oni woman with a single ivory horn on her forehead, long dark indigo hair with red streaks, red eyes, red marks on her cheeks, a small fang, a dark kimono-style collar with a crimson sash, a disciplined calm look. Badge: a black lacquer ring with crimson (#D7263D) and gold torii-gate ornaments, a warm lantern glow behind her. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### 🌑 Dark Elf — the adversary

```
Game icon, round badge, centred, head and shoulders, front view. A dark elf woman with grey-violet skin, long silver hair, grey eyes, long pointed ears, gold drop earrings, a knowing half-smile with a small fang, a high black-and-gold collar, holding a slim silver monocle lens near one eye. Badge: an obsidian ring with violet (#7B4FD6) and thin silver edges, small thorn ornaments, a cold moonlit glow behind her. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### 🐉 Veldora — the human

```
Game icon, round badge, centred, front view. The head of a vast wise storm dragon, amused expression, one large golden slit eye, dark indigo scales, small lightning crackling around its horns. Badge: a storm-gold ring (#D4AF37) with indigo lightning filigree, swirling storm clouds behind. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### ✨ High Elf — ascended Elf (Character Reference: the Elf icon)

```
Game icon, round badge, centred, head and shoulders, front view. The same elf woman, ascended: long blonde hair with luminous teal-white strands, aqua-green eyes, an emerald and white-gold tiara with a central gem, green and white high-collared robe, serene expression, small motes of light around her. Badge: a white-gold ring (#F5E6B8) with emerald inlays and a soft radiant halo, a pale green-white aura behind her. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### ⚔️ High Orc — ascended Orc (Character Reference: the Orc icon)

```
Game icon, round badge, centred, head and shoulders, front view. The same green orc, ascended: long brown hair, a horned helm of bone and iron, heavier tusks, a scar over one brow, proud fierce expression, a fur-trimmed mantle over iron armour. Badge: a blackened iron ring with glowing molten-gold seams (#E8A33D) and a small crown at the top, warm forge light behind him. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### 💎 Named Slime — ascended Slime (Character Reference: the Slime icon)

```
Game icon, round badge, centred, front view. The same round slime with big red eyes and blush marks, ascended: its body is now a clear faceted aquamarine crystal (#5EEAD4) with light refracting inside, a small glowing star-shaped mark on its forehead, a happy proud smile. Badge: a ring of crystal and platinum cut like a gemstone, sparkling highlights, a cool aqua glow behind. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### 🔥 Oni — ascended Kijin (Character Reference: the Kijin icon)

```
Game icon, round badge, centred, head and shoulders, front view. The same oni woman, ascended: her single ivory horn is longer and glows faint flame-gold, hair streaks burning crimson-orange (#FF6B35), red eyes, red cheek marks, a formal crimson and gold haori collar, small embers drifting around her, a calm commanding look. Badge: a black lacquer ring with gold flame filigree and a lit torii ornament, a warm firelight glow behind her. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

### 🌘 Arch Dark Elf — ascended Dark Elf (Character Reference: the Dark Elf icon)

```
Game icon, round badge, centred, head and shoulders, front view. The same dark elf woman, ascended: silver hair with violet-black tips, a dark silver circlet with a crescent moon, grey-violet skin, gold drop earrings, a cool dangerous smile, a high collar with violet lining, a black eclipse with a silver corona behind her head. Badge: an obsidian and silver ring with eclipse motifs (#2E1065), small violet seal sigils floating at its edge. Polished anime style, crisp clean line art, cel shading with soft gradients, rim light from upper left. Plain white background outside the badge, no text, strong silhouette readable at 32 px.
```

## What I check when they come back

1. No text or fake letters anywhere.
2. Same badge size and position in every icon (I overlay them).
3. Still recognisable at 32 px and 64 px; each race's colour distinct for colour-blind viewers.
4. Each ascended icon reads as its base creature, evolved.
5. Continuity with the isekai portraits and the race cards.

Icons that pass go to `assets/portraits/<race>.png`.
