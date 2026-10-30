//! What a human could not see for what it is, and text as a human is shown it: the one rule the
//! approval card, the terminal app, the wire, `mnemorphd`'s printouts and the board's port share.

/// The characters [`hidden`] escapes beyond Unicode's controls (Cc), as sorted, inclusive ranges:
/// every format character (Cf), the line and paragraph separators (Zl, Zp) and every
/// default-ignorable code point — each draws nothing, or reorders what is drawn, so a text holding
/// one reads as another. The space separators (Zs) stay as written: each draws as space.
/// The shared vectors (`tests/shown-vectors.json`) list the same ranges, and the Rust and the
/// board's tests both check every code point against them, so the rules cannot drift.
pub const HIDDEN: &[(char, char)] = &[
    // SOFT HYPHEN: drawn only at a line break.
    ('\u{AD}', '\u{AD}'),
    // COMBINING GRAPHEME JOINER: draws nothing.
    ('\u{34F}', '\u{34F}'),
    // Arabic number and end-of-ayah signs (Cf): marks that span the text after them.
    ('\u{600}', '\u{605}'),
    // ARABIC LETTER MARK: a bidi control.
    ('\u{61C}', '\u{61C}'),
    ('\u{6DD}', '\u{6DD}'),
    // SYRIAC ABBREVIATION MARK (Cf).
    ('\u{70F}', '\u{70F}'),
    // Arabic pound and piastre marks above, disputed end of ayah (Cf).
    ('\u{890}', '\u{891}'),
    ('\u{8E2}', '\u{8E2}'),
    // HANGUL CHOSEONG and JUNGSEONG FILLER: blank letters, the classic invisible name.
    ('\u{115F}', '\u{1160}'),
    // KHMER VOWEL INHERENT AQ and AA: default-ignorable, draw nothing.
    ('\u{17B4}', '\u{17B5}'),
    // MONGOLIAN FREE VARIATION SELECTORS and VOWEL SEPARATOR: invisible.
    ('\u{180B}', '\u{180F}'),
    // ZERO WIDTH SPACE, NON-JOINER and JOINER, LEFT-TO-RIGHT and RIGHT-TO-LEFT MARK. The joiner
    // too, though it glues emoji sequences: what a human approves is exact, so a family emoji
    // shows its joiners.
    ('\u{200B}', '\u{200F}'),
    // LINE and PARAGRAPH SEPARATOR, then the bidi embeddings and overrides (U+202A–202E).
    ('\u{2028}', '\u{202E}'),
    // WORD JOINER, the invisible operators, the bidi isolates (U+2066–2069) and the deprecated
    // format characters (U+206A–206F).
    ('\u{2060}', '\u{206F}'),
    // HANGUL FILLER: a blank letter.
    ('\u{3164}', '\u{3164}'),
    // VARIATION SELECTORS 1–16. U+FE0F too, though emoji take it: after a letter it draws
    // nothing, so `rm` and `rm` + U+FE0F would read alike. Prose keeps it ([`kept_in_prose`]).
    ('\u{FE00}', '\u{FE0F}'),
    // ZERO WIDTH NO-BREAK SPACE (the byte-order mark).
    ('\u{FEFF}', '\u{FEFF}'),
    // HALFWIDTH HANGUL FILLER: a blank letter.
    ('\u{FFA0}', '\u{FFA0}'),
    // Reserved default-ignorables, then the interlinear annotation controls (U+FFF9–FFFB).
    ('\u{FFF0}', '\u{FFFB}'),
    // Kaithi number signs (Cf).
    ('\u{110BD}', '\u{110BD}'),
    ('\u{110CD}', '\u{110CD}'),
    // Egyptian hieroglyph format controls (Cf).
    ('\u{13430}', '\u{1343F}'),
    // Shorthand format controls (Cf).
    ('\u{1BCA0}', '\u{1BCA3}'),
    // Musical symbol beams, ties and phrases (Cf): draw nothing alone.
    ('\u{1D173}', '\u{1D17A}'),
    // The tag characters (U+E0000–E007F): invisible, and they spell ASCII — a text smuggled
    // inside a text. Then the variation selectors supplement (U+E0100–E01EF) and the rest of the
    // reserved default-ignorable plane block.
    ('\u{E0000}', '\u{E0FFF}'),
];

/// Whether `c` is a character a human could not see for what it is: a control character (Cc),
/// or [`invisible`].
#[must_use]
pub fn hidden(c: char) -> bool {
    c.is_control() || invisible(c)
}

/// Whether `c` is one of [`HIDDEN`]: a format character, a bidi control, a line or paragraph
/// separator, a default-ignorable code point.
#[must_use]
pub fn invisible(c: char) -> bool {
    within(HIDDEN, c)
}

/// Whether `c` falls in one of `ranges`, sorted and inclusive.
fn within(ranges: &[(char, char)], c: char) -> bool {
    ranges
        .binary_search_by(|&(lo, hi)| {
            if hi < c {
                std::cmp::Ordering::Less
            } else if lo > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Whether prose — the transcript's text, never an act or a name — keeps `c` as written though
/// it is [`hidden`]: U+FE0F alone, which only asks for the emoji form of the character before it.
/// What a human approves, and every one-line field, escapes it.
#[must_use]
pub fn kept_in_prose(c: char) -> bool {
    c == '\u{FE0F}'
}

/// The space separators (Zs) other than U+0020: each draws as a space, so on the card and in a
/// one-line field — a name, a path, an act — a no-break or ideographic space would pass for the
/// plain one. U+1680 OGHAM SPACE MARK draws a dash, and stays.
pub const SPACES: &[(char, char)] = &[
    ('\u{A0}', '\u{A0}'),
    ('\u{2000}', '\u{200A}'),
    ('\u{202F}', '\u{202F}'),
    ('\u{205F}', '\u{205F}'),
    ('\u{3000}', '\u{3000}'),
];

/// How many combining marks one base character keeps on the card; each one past it is escaped,
/// so a stack of marks can never draw over the text around it.
pub const MARKS_KEPT: usize = 2;

/// The combining marks (Unicode Mn and Me, as of Unicode 16), sorted and inclusive; the shared
/// vectors list the same ranges.
pub const MARKS: &[(char, char)] = &[
    ('\u{300}', '\u{36F}'),
    ('\u{483}', '\u{489}'),
    ('\u{591}', '\u{5BD}'),
    ('\u{5BF}', '\u{5BF}'),
    ('\u{5C1}', '\u{5C2}'),
    ('\u{5C4}', '\u{5C5}'),
    ('\u{5C7}', '\u{5C7}'),
    ('\u{610}', '\u{61A}'),
    ('\u{64B}', '\u{65F}'),
    ('\u{670}', '\u{670}'),
    ('\u{6D6}', '\u{6DC}'),
    ('\u{6DF}', '\u{6E4}'),
    ('\u{6E7}', '\u{6E8}'),
    ('\u{6EA}', '\u{6ED}'),
    ('\u{711}', '\u{711}'),
    ('\u{730}', '\u{74A}'),
    ('\u{7A6}', '\u{7B0}'),
    ('\u{7EB}', '\u{7F3}'),
    ('\u{7FD}', '\u{7FD}'),
    ('\u{816}', '\u{819}'),
    ('\u{81B}', '\u{823}'),
    ('\u{825}', '\u{827}'),
    ('\u{829}', '\u{82D}'),
    ('\u{859}', '\u{85B}'),
    ('\u{897}', '\u{89F}'),
    ('\u{8CA}', '\u{8E1}'),
    ('\u{8E3}', '\u{902}'),
    ('\u{93A}', '\u{93A}'),
    ('\u{93C}', '\u{93C}'),
    ('\u{941}', '\u{948}'),
    ('\u{94D}', '\u{94D}'),
    ('\u{951}', '\u{957}'),
    ('\u{962}', '\u{963}'),
    ('\u{981}', '\u{981}'),
    ('\u{9BC}', '\u{9BC}'),
    ('\u{9C1}', '\u{9C4}'),
    ('\u{9CD}', '\u{9CD}'),
    ('\u{9E2}', '\u{9E3}'),
    ('\u{9FE}', '\u{9FE}'),
    ('\u{A01}', '\u{A02}'),
    ('\u{A3C}', '\u{A3C}'),
    ('\u{A41}', '\u{A42}'),
    ('\u{A47}', '\u{A48}'),
    ('\u{A4B}', '\u{A4D}'),
    ('\u{A51}', '\u{A51}'),
    ('\u{A70}', '\u{A71}'),
    ('\u{A75}', '\u{A75}'),
    ('\u{A81}', '\u{A82}'),
    ('\u{ABC}', '\u{ABC}'),
    ('\u{AC1}', '\u{AC5}'),
    ('\u{AC7}', '\u{AC8}'),
    ('\u{ACD}', '\u{ACD}'),
    ('\u{AE2}', '\u{AE3}'),
    ('\u{AFA}', '\u{AFF}'),
    ('\u{B01}', '\u{B01}'),
    ('\u{B3C}', '\u{B3C}'),
    ('\u{B3F}', '\u{B3F}'),
    ('\u{B41}', '\u{B44}'),
    ('\u{B4D}', '\u{B4D}'),
    ('\u{B55}', '\u{B56}'),
    ('\u{B62}', '\u{B63}'),
    ('\u{B82}', '\u{B82}'),
    ('\u{BC0}', '\u{BC0}'),
    ('\u{BCD}', '\u{BCD}'),
    ('\u{C00}', '\u{C00}'),
    ('\u{C04}', '\u{C04}'),
    ('\u{C3C}', '\u{C3C}'),
    ('\u{C3E}', '\u{C40}'),
    ('\u{C46}', '\u{C48}'),
    ('\u{C4A}', '\u{C4D}'),
    ('\u{C55}', '\u{C56}'),
    ('\u{C62}', '\u{C63}'),
    ('\u{C81}', '\u{C81}'),
    ('\u{CBC}', '\u{CBC}'),
    ('\u{CBF}', '\u{CBF}'),
    ('\u{CC6}', '\u{CC6}'),
    ('\u{CCC}', '\u{CCD}'),
    ('\u{CE2}', '\u{CE3}'),
    ('\u{D00}', '\u{D01}'),
    ('\u{D3B}', '\u{D3C}'),
    ('\u{D41}', '\u{D44}'),
    ('\u{D4D}', '\u{D4D}'),
    ('\u{D62}', '\u{D63}'),
    ('\u{D81}', '\u{D81}'),
    ('\u{DCA}', '\u{DCA}'),
    ('\u{DD2}', '\u{DD4}'),
    ('\u{DD6}', '\u{DD6}'),
    ('\u{E31}', '\u{E31}'),
    ('\u{E34}', '\u{E3A}'),
    ('\u{E47}', '\u{E4E}'),
    ('\u{EB1}', '\u{EB1}'),
    ('\u{EB4}', '\u{EBC}'),
    ('\u{EC8}', '\u{ECE}'),
    ('\u{F18}', '\u{F19}'),
    ('\u{F35}', '\u{F35}'),
    ('\u{F37}', '\u{F37}'),
    ('\u{F39}', '\u{F39}'),
    ('\u{F71}', '\u{F7E}'),
    ('\u{F80}', '\u{F84}'),
    ('\u{F86}', '\u{F87}'),
    ('\u{F8D}', '\u{F97}'),
    ('\u{F99}', '\u{FBC}'),
    ('\u{FC6}', '\u{FC6}'),
    ('\u{102D}', '\u{1030}'),
    ('\u{1032}', '\u{1037}'),
    ('\u{1039}', '\u{103A}'),
    ('\u{103D}', '\u{103E}'),
    ('\u{1058}', '\u{1059}'),
    ('\u{105E}', '\u{1060}'),
    ('\u{1071}', '\u{1074}'),
    ('\u{1082}', '\u{1082}'),
    ('\u{1085}', '\u{1086}'),
    ('\u{108D}', '\u{108D}'),
    ('\u{109D}', '\u{109D}'),
    ('\u{135D}', '\u{135F}'),
    ('\u{1712}', '\u{1714}'),
    ('\u{1732}', '\u{1733}'),
    ('\u{1752}', '\u{1753}'),
    ('\u{1772}', '\u{1773}'),
    ('\u{17B4}', '\u{17B5}'),
    ('\u{17B7}', '\u{17BD}'),
    ('\u{17C6}', '\u{17C6}'),
    ('\u{17C9}', '\u{17D3}'),
    ('\u{17DD}', '\u{17DD}'),
    ('\u{180B}', '\u{180D}'),
    ('\u{180F}', '\u{180F}'),
    ('\u{1885}', '\u{1886}'),
    ('\u{18A9}', '\u{18A9}'),
    ('\u{1920}', '\u{1922}'),
    ('\u{1927}', '\u{1928}'),
    ('\u{1932}', '\u{1932}'),
    ('\u{1939}', '\u{193B}'),
    ('\u{1A17}', '\u{1A18}'),
    ('\u{1A1B}', '\u{1A1B}'),
    ('\u{1A56}', '\u{1A56}'),
    ('\u{1A58}', '\u{1A5E}'),
    ('\u{1A60}', '\u{1A60}'),
    ('\u{1A62}', '\u{1A62}'),
    ('\u{1A65}', '\u{1A6C}'),
    ('\u{1A73}', '\u{1A7C}'),
    ('\u{1A7F}', '\u{1A7F}'),
    ('\u{1AB0}', '\u{1ADD}'),
    ('\u{1AE0}', '\u{1AEB}'),
    ('\u{1B00}', '\u{1B03}'),
    ('\u{1B34}', '\u{1B34}'),
    ('\u{1B36}', '\u{1B3A}'),
    ('\u{1B3C}', '\u{1B3C}'),
    ('\u{1B42}', '\u{1B42}'),
    ('\u{1B6B}', '\u{1B73}'),
    ('\u{1B80}', '\u{1B81}'),
    ('\u{1BA2}', '\u{1BA5}'),
    ('\u{1BA8}', '\u{1BA9}'),
    ('\u{1BAB}', '\u{1BAD}'),
    ('\u{1BE6}', '\u{1BE6}'),
    ('\u{1BE8}', '\u{1BE9}'),
    ('\u{1BED}', '\u{1BED}'),
    ('\u{1BEF}', '\u{1BF1}'),
    ('\u{1C2C}', '\u{1C33}'),
    ('\u{1C36}', '\u{1C37}'),
    ('\u{1CD0}', '\u{1CD2}'),
    ('\u{1CD4}', '\u{1CE0}'),
    ('\u{1CE2}', '\u{1CE8}'),
    ('\u{1CED}', '\u{1CED}'),
    ('\u{1CF4}', '\u{1CF4}'),
    ('\u{1CF8}', '\u{1CF9}'),
    ('\u{1DC0}', '\u{1DFF}'),
    ('\u{20D0}', '\u{20F0}'),
    ('\u{2CEF}', '\u{2CF1}'),
    ('\u{2D7F}', '\u{2D7F}'),
    ('\u{2DE0}', '\u{2DFF}'),
    ('\u{302A}', '\u{302D}'),
    ('\u{3099}', '\u{309A}'),
    ('\u{A66F}', '\u{A672}'),
    ('\u{A674}', '\u{A67D}'),
    ('\u{A69E}', '\u{A69F}'),
    ('\u{A6F0}', '\u{A6F1}'),
    ('\u{A802}', '\u{A802}'),
    ('\u{A806}', '\u{A806}'),
    ('\u{A80B}', '\u{A80B}'),
    ('\u{A825}', '\u{A826}'),
    ('\u{A82C}', '\u{A82C}'),
    ('\u{A8C4}', '\u{A8C5}'),
    ('\u{A8E0}', '\u{A8F1}'),
    ('\u{A8FF}', '\u{A8FF}'),
    ('\u{A926}', '\u{A92D}'),
    ('\u{A947}', '\u{A951}'),
    ('\u{A980}', '\u{A982}'),
    ('\u{A9B3}', '\u{A9B3}'),
    ('\u{A9B6}', '\u{A9B9}'),
    ('\u{A9BC}', '\u{A9BD}'),
    ('\u{A9E5}', '\u{A9E5}'),
    ('\u{AA29}', '\u{AA2E}'),
    ('\u{AA31}', '\u{AA32}'),
    ('\u{AA35}', '\u{AA36}'),
    ('\u{AA43}', '\u{AA43}'),
    ('\u{AA4C}', '\u{AA4C}'),
    ('\u{AA7C}', '\u{AA7C}'),
    ('\u{AAB0}', '\u{AAB0}'),
    ('\u{AAB2}', '\u{AAB4}'),
    ('\u{AAB7}', '\u{AAB8}'),
    ('\u{AABE}', '\u{AABF}'),
    ('\u{AAC1}', '\u{AAC1}'),
    ('\u{AAEC}', '\u{AAED}'),
    ('\u{AAF6}', '\u{AAF6}'),
    ('\u{ABE5}', '\u{ABE5}'),
    ('\u{ABE8}', '\u{ABE8}'),
    ('\u{ABED}', '\u{ABED}'),
    ('\u{FB1E}', '\u{FB1E}'),
    ('\u{FE00}', '\u{FE0F}'),
    ('\u{FE20}', '\u{FE2F}'),
    ('\u{101FD}', '\u{101FD}'),
    ('\u{102E0}', '\u{102E0}'),
    ('\u{10376}', '\u{1037A}'),
    ('\u{10A01}', '\u{10A03}'),
    ('\u{10A05}', '\u{10A06}'),
    ('\u{10A0C}', '\u{10A0F}'),
    ('\u{10A38}', '\u{10A3A}'),
    ('\u{10A3F}', '\u{10A3F}'),
    ('\u{10AE5}', '\u{10AE6}'),
    ('\u{10D24}', '\u{10D27}'),
    ('\u{10D69}', '\u{10D6D}'),
    ('\u{10EAB}', '\u{10EAC}'),
    ('\u{10EFA}', '\u{10EFF}'),
    ('\u{10F46}', '\u{10F50}'),
    ('\u{10F82}', '\u{10F85}'),
    ('\u{11001}', '\u{11001}'),
    ('\u{11038}', '\u{11046}'),
    ('\u{11070}', '\u{11070}'),
    ('\u{11073}', '\u{11074}'),
    ('\u{1107F}', '\u{11081}'),
    ('\u{110B3}', '\u{110B6}'),
    ('\u{110B9}', '\u{110BA}'),
    ('\u{110C2}', '\u{110C2}'),
    ('\u{11100}', '\u{11102}'),
    ('\u{11127}', '\u{1112B}'),
    ('\u{1112D}', '\u{11134}'),
    ('\u{11173}', '\u{11173}'),
    ('\u{11180}', '\u{11181}'),
    ('\u{111B6}', '\u{111BE}'),
    ('\u{111C9}', '\u{111CC}'),
    ('\u{111CF}', '\u{111CF}'),
    ('\u{1122F}', '\u{11231}'),
    ('\u{11234}', '\u{11234}'),
    ('\u{11236}', '\u{11237}'),
    ('\u{1123E}', '\u{1123E}'),
    ('\u{11241}', '\u{11241}'),
    ('\u{112DF}', '\u{112DF}'),
    ('\u{112E3}', '\u{112EA}'),
    ('\u{11300}', '\u{11301}'),
    ('\u{1133B}', '\u{1133C}'),
    ('\u{11340}', '\u{11340}'),
    ('\u{11366}', '\u{1136C}'),
    ('\u{11370}', '\u{11374}'),
    ('\u{113BB}', '\u{113C0}'),
    ('\u{113CE}', '\u{113CE}'),
    ('\u{113D0}', '\u{113D0}'),
    ('\u{113D2}', '\u{113D2}'),
    ('\u{113E1}', '\u{113E2}'),
    ('\u{11438}', '\u{1143F}'),
    ('\u{11442}', '\u{11444}'),
    ('\u{11446}', '\u{11446}'),
    ('\u{1145E}', '\u{1145E}'),
    ('\u{114B3}', '\u{114B8}'),
    ('\u{114BA}', '\u{114BA}'),
    ('\u{114BF}', '\u{114C0}'),
    ('\u{114C2}', '\u{114C3}'),
    ('\u{115B2}', '\u{115B5}'),
    ('\u{115BC}', '\u{115BD}'),
    ('\u{115BF}', '\u{115C0}'),
    ('\u{115DC}', '\u{115DD}'),
    ('\u{11633}', '\u{1163A}'),
    ('\u{1163D}', '\u{1163D}'),
    ('\u{1163F}', '\u{11640}'),
    ('\u{116AB}', '\u{116AB}'),
    ('\u{116AD}', '\u{116AD}'),
    ('\u{116B0}', '\u{116B5}'),
    ('\u{116B7}', '\u{116B7}'),
    ('\u{1171D}', '\u{1171D}'),
    ('\u{1171F}', '\u{1171F}'),
    ('\u{11722}', '\u{11725}'),
    ('\u{11727}', '\u{1172B}'),
    ('\u{1182F}', '\u{11837}'),
    ('\u{11839}', '\u{1183A}'),
    ('\u{1193B}', '\u{1193C}'),
    ('\u{1193E}', '\u{1193E}'),
    ('\u{11943}', '\u{11943}'),
    ('\u{119D4}', '\u{119D7}'),
    ('\u{119DA}', '\u{119DB}'),
    ('\u{119E0}', '\u{119E0}'),
    ('\u{11A01}', '\u{11A0A}'),
    ('\u{11A33}', '\u{11A38}'),
    ('\u{11A3B}', '\u{11A3E}'),
    ('\u{11A47}', '\u{11A47}'),
    ('\u{11A51}', '\u{11A56}'),
    ('\u{11A59}', '\u{11A5B}'),
    ('\u{11A8A}', '\u{11A96}'),
    ('\u{11A98}', '\u{11A99}'),
    ('\u{11B60}', '\u{11B60}'),
    ('\u{11B62}', '\u{11B64}'),
    ('\u{11B66}', '\u{11B66}'),
    ('\u{11C30}', '\u{11C36}'),
    ('\u{11C38}', '\u{11C3D}'),
    ('\u{11C3F}', '\u{11C3F}'),
    ('\u{11C92}', '\u{11CA7}'),
    ('\u{11CAA}', '\u{11CB0}'),
    ('\u{11CB2}', '\u{11CB3}'),
    ('\u{11CB5}', '\u{11CB6}'),
    ('\u{11D31}', '\u{11D36}'),
    ('\u{11D3A}', '\u{11D3A}'),
    ('\u{11D3C}', '\u{11D3D}'),
    ('\u{11D3F}', '\u{11D45}'),
    ('\u{11D47}', '\u{11D47}'),
    ('\u{11D90}', '\u{11D91}'),
    ('\u{11D95}', '\u{11D95}'),
    ('\u{11D97}', '\u{11D97}'),
    ('\u{11EF3}', '\u{11EF4}'),
    ('\u{11F00}', '\u{11F01}'),
    ('\u{11F36}', '\u{11F3A}'),
    ('\u{11F40}', '\u{11F40}'),
    ('\u{11F42}', '\u{11F42}'),
    ('\u{11F5A}', '\u{11F5A}'),
    ('\u{13440}', '\u{13440}'),
    ('\u{13447}', '\u{13455}'),
    ('\u{1611E}', '\u{16129}'),
    ('\u{1612D}', '\u{1612F}'),
    ('\u{16AF0}', '\u{16AF4}'),
    ('\u{16B30}', '\u{16B36}'),
    ('\u{16F4F}', '\u{16F4F}'),
    ('\u{16F8F}', '\u{16F92}'),
    ('\u{16FE4}', '\u{16FE4}'),
    ('\u{1BC9D}', '\u{1BC9E}'),
    ('\u{1CF00}', '\u{1CF2D}'),
    ('\u{1CF30}', '\u{1CF46}'),
    ('\u{1D167}', '\u{1D169}'),
    ('\u{1D17B}', '\u{1D182}'),
    ('\u{1D185}', '\u{1D18B}'),
    ('\u{1D1AA}', '\u{1D1AD}'),
    ('\u{1D242}', '\u{1D244}'),
    ('\u{1DA00}', '\u{1DA36}'),
    ('\u{1DA3B}', '\u{1DA6C}'),
    ('\u{1DA75}', '\u{1DA75}'),
    ('\u{1DA84}', '\u{1DA84}'),
    ('\u{1DA9B}', '\u{1DA9F}'),
    ('\u{1DAA1}', '\u{1DAAF}'),
    ('\u{1E000}', '\u{1E006}'),
    ('\u{1E008}', '\u{1E018}'),
    ('\u{1E01B}', '\u{1E021}'),
    ('\u{1E023}', '\u{1E024}'),
    ('\u{1E026}', '\u{1E02A}'),
    ('\u{1E08F}', '\u{1E08F}'),
    ('\u{1E130}', '\u{1E136}'),
    ('\u{1E2AE}', '\u{1E2AE}'),
    ('\u{1E2EC}', '\u{1E2EF}'),
    ('\u{1E4EC}', '\u{1E4EF}'),
    ('\u{1E5EE}', '\u{1E5EF}'),
    ('\u{1E6E3}', '\u{1E6E3}'),
    ('\u{1E6E6}', '\u{1E6E6}'),
    ('\u{1E6EE}', '\u{1E6EF}'),
    ('\u{1E6F5}', '\u{1E6F5}'),
    ('\u{1E8D0}', '\u{1E8D6}'),
    ('\u{1E944}', '\u{1E94A}'),
    ('\u{E0100}', '\u{E01EF}'),
];

/// Whether `c` is a combining mark ([`MARKS`]).
#[must_use]
pub fn mark(c: char) -> bool {
    within(MARKS, c)
}

/// The card's rule, character by character: [`hidden`], a [`SPACES`] space, or a combining mark
/// past the [`MARKS_KEPT`] its base keeps. It counts the marks since the last base, so it walks a
/// text in order.
#[derive(Debug, Default, Clone, Copy)]
pub struct Strict {
    marks: usize,
}

impl Strict {
    /// Whether the card escapes `c`, the next character of the text.
    pub fn escapes(&mut self, c: char) -> bool {
        if mark(c) {
            self.marks += 1;
            hidden(c) || self.marks > MARKS_KEPT
        } else {
            self.marks = 0;
            hidden(c) || within(SPACES, c)
        }
    }
}

/// A text a model's choice reaches, as a human is shown it on the card and in a one-line field:
/// every character [`Strict`] names escaped as `\u{…}`, and a backslash too, so an escape can
/// never be mistaken for one written in a name.
#[must_use]
pub fn shown(text: &str) -> String {
    let mut strict = Strict::default();
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if strict.escapes(c) || c == '\\' {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}
