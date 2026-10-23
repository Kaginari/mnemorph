"""Draw the race cards: the hex portraits framed with the words of the race law (docs/02-races.md).

Usage: python3 tools/cards.py   (from the repository root; needs Pillow)
Writes assets/cards/<race>.png and back.png, 600x840.
"""
from PIL import Image, ImageDraw, ImageFilter, ImageFont, ImageChops

REPO, FONTS, OUT = '.', 'assets/fonts', 'assets/cards'
W, H = 1200, 1680          # drawn at 2x, saved at 600x840
CLASSES = ['read', 'write', 'outward', 'destructive', 'dispatch', 'seal']

CARDS = [
    dict(key='rimuru', name='Rimuru', tier=1, kind='THE THRONE', color=(63, 169, 245), gem=(245, 230, 184),
         epithet='the throne', duty='Understands the human, plans, and answers.',
         powers={'read', 'write', 'dispatch'}, soil='the plan and the dimension manifest',
         line=('COMMANDS', 'Elves, Kijin and Dark Elves'), meter=('Asks done', 80)),
    dict(key='elf', name='Elf', tier=1, kind='BASE RACE', color=(47, 163, 107), gem=(214, 178, 90),
         epithet="the lane's planner and voice", duty='Plans one lane of work across domains.',
         powers={'read', 'write', 'dispatch'}, soil='its own doc',
         line=('COMMANDS', 'its Orcs'), meter=('Plans landed', 80)),
    dict(key='orc', name='Orc', tier=1, kind='BASE RACE', color=(201, 130, 43), gem=(201, 130, 43),
         epithet='the domain and its gate', duty='Owns a domain: writes the test first, judges the landing.',
         powers={'read', 'write', 'dispatch'}, soil="its doc and its slimes' verify lines",
         line=('COMMANDS', 'its Slimes; other Orcs sideways'), meter=('Gate right', 90)),
    dict(key='slime', name='Slime', tier=1, kind='BASE RACE', color=(25, 195, 176), gem=(200, 220, 230),
         epithet='the ground truth of a zone', duty='Changes the code of its one zone, with its doc.',
         powers={'read', 'write'}, soil='its zone — the only race that writes code',
         line=('COMMANDS', 'nobody'), meter=('Landed', 80)),
    dict(key='kijin', name='Kijin', tier=1, kind='BASE RACE', color=(215, 38, 61), gem=(222, 178, 80),
         epithet='the hands that reach outside', duty='Everything that leaves the machine: CI, release, deploy, PRs, intel.',
         powers={'read', 'write', 'outward', 'destructive'}, soil='its post — ci, release, deploy, forge, scout',
         line=('GATE', 'destructive acts always wait for the human'), meter=('Outward clean', 90)),
    dict(key='dark-elf', name='Dark Elf', tier=1, kind='BASE RACE', color=(123, 79, 214), gem=(200, 205, 220),
         epithet='the adversary', duty='Attacks the code, and proves every weakness it claims.',
         powers={'read', 'write', 'seal'}, soil='its proving ground',
         line=('SEALS', 'proposed; the human confirms'), meter=('Upheld', 80)),
    dict(key='high-elf', name='High Elf', tier=2, kind='ASCENDED FORM', color=(245, 230, 184), gem=(47, 163, 107),
         epithet='ascended Elf', duty='Orders work in another lane — through that lane\'s Elf, never past it.',
         powers={'read', 'write', 'dispatch'}, soil='its own doc',
         line=('RISES', '20 reports · plans landed ≥ 85% · re-asks ≤ 10%'), meter=('Plans landed', 80)),
    dict(key='high-orc', name='High Orc', tier=2, kind='ASCENDED FORM', color=(232, 168, 60), gem=(90, 90, 100),
         epithet='ascended Orc', duty="Amends — never weakens — its slimes' verify lines mid-turn.",
         powers={'read', 'write', 'dispatch'}, soil="its doc and its slimes' verify lines",
         line=('RISES', '20 reports · gate right ≥ 90% · holes ≤ 10%'), meter=('Gate right', 90)),
    dict(key='named-slime', name='Named Slime', tier=2, kind='ASCENDED FORM', color=(160, 232, 240), gem=(230, 236, 245),
         epithet='ascended Slime', duty='Adds verify lines to its own doc; +50% step budget.',
         powers={'read', 'write'}, soil='its zone, first claim on an open neighbour',
         line=('RISES', '20 reports · landed ≥ 90% · gates ≥ 90%'), meter=('Landed', 80)),
    dict(key='oni', name='Oni', tier=2, kind='ASCENDED FORM', color=(232, 176, 74), gem=(215, 38, 61),
         epithet='ascended Kijin', duty='Acts under Oni envelopes: standing permissions the human signs once.',
         powers={'read', 'write', 'outward', 'destructive'}, soil='its post',
         line=('RISES', '30 outward acts · ≥ 95% clean · no rollback'), meter=('Outward clean', 90)),
    dict(key='arch-dark-elf', name='Arch Dark Elf', tier=2, kind='ASCENDED FORM', color=(196, 204, 228), gem=(123, 79, 214),
         epithet='ascended Dark Elf', duty='Its seals go straight to the human; runs background fuzz campaigns.',
         powers={'read', 'write', 'seal'}, soil='its proving ground',
         line=('RISES', '20 findings · upheld ≥ 80% · precision ≥ 85%'), meter=('Upheld', 80)),
    dict(key='veldora', name='Veldora', tier=3, kind='THE HUMAN', color=(212, 175, 55), gem=(110, 90, 210),
         epithet='the storm dragon', duty='Sets the asks, signs the envelopes, holds the destructive key.',
         powers=set(CLASSES), soil='the whole world',
         line=('GATE', 'every seal and every destructive act'), meter=None),
]


def font(name, size):
    f = ImageFont.truetype(f'{FONTS}/{name}', size)
    if name == 'Cinzel.ttf':
        f.set_variation_by_axes([700])
    return f


def rr_mask(box, r):
    m = Image.new('L', (W, H), 0)
    d = ImageDraw.Draw(m)
    x0, y0, x1, y1 = box
    d.rectangle((x0 + r, y0, x1 - r, y1), fill=255)
    d.rectangle((x0, y0 + r, x1, y1 - r), fill=255)
    for cx, cy in ((x0, y0), (x1 - 2 * r, y0), (x0, y1 - 2 * r), (x1 - 2 * r, y1 - 2 * r)):
        d.ellipse((cx, cy, cx + 2 * r, cy + 2 * r), fill=255)
    return m


def paint(img, mask, color, alpha=255):
    layer = Image.new('RGBA', (W, H), color + (255,))
    if alpha < 255:
        mask = mask.point(lambda v: v * alpha // 255)
    img.alpha_composite(Image.composite(layer, Image.new('RGBA', (W, H), (0, 0, 0, 0)), mask))


def ring(img, box, r, width, color, alpha=255):
    x0, y0, x1, y1 = box
    m = ImageChops.subtract(rr_mask(box, r), rr_mask((x0 + width, y0 + width, x1 - width, y1 - width), max(r - width, 1)))
    paint(img, m, color, alpha)


def glow(img, mask, color, radius, strength=1.0):
    g = mask.filter(ImageFilter.GaussianBlur(radius)).point(lambda v: min(255, int(v * strength)))
    paint(img, g, color)


def mix(a, b, t):
    return tuple(int(a[i] * (1 - t) + b[i] * t) for i in range(3))


def text_mask(xy, text, f, anchor_center=True):
    m = Image.new('L', (W, H), 0)
    tw, th = f.getsize(text)
    x, y = xy
    if anchor_center:
        x -= tw // 2
    ImageDraw.Draw(m).text((x, y), text, font=f, fill=255)
    return m


def wrap(text, f, width):
    words, lines, cur = text.split(), [], ''
    for w in words:
        t = (cur + ' ' + w).strip()
        if f.getsize(t)[0] <= width:
            cur = t
        else:
            lines.append(cur)
            cur = w
    lines.append(cur)
    return lines


def gem(img, cx, cy, s, color):
    m = Image.new('L', (W, H), 0)
    ImageDraw.Draw(m).polygon([(cx, cy - s), (cx + s * 0.7, cy), (cx, cy + s), (cx - s * 0.7, cy)], fill=255)
    glow(img, m, color, 10, 1.4)
    paint(img, m, color)
    hi = Image.new('L', (W, H), 0)
    ImageDraw.Draw(hi).polygon([(cx, cy - s * 0.8), (cx + s * 0.3, cy - s * 0.1), (cx, cy), (cx - s * 0.3, cy - s * 0.1)], fill=255)
    paint(img, hi, (255, 255, 255), 150)


def card(c):
    col, gemc = c['color'], c['gem']
    img = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    body = (20, 20, W - 20, H - 20)
    shape = rr_mask(body, 56)
    # the panel: a deep vertical gradient, tinted by the race
    grad = Image.new('RGBA', (W, H))
    gd = ImageDraw.Draw(grad)
    top, bottom = mix((10, 16, 30), col, 0.10), (6, 9, 18)
    for y in range(H):
        gd.line((0, y, W, y), fill=mix(top, bottom, y / H) + (255,))
    # a faint circuit grid
    for x in range(60, W, 60):
        gd.line((x, 0, x, H), fill=mix(top, col, 0.10) + (255,))
    for y in range(60, H, 60):
        gd.line((0, y, W, y), fill=mix(top, col, 0.07) + (255,))
    img.alpha_composite(Image.composite(grad, img, shape))
    # the frame: outer band, glow, inner hairline, corner notches
    glow(img, ImageChops.subtract(shape, rr_mask((34, 34, W - 34, H - 34), 44)), col, 14, 1.2)
    ring(img, body, 56, 10, col)
    ring(img, (44, 44, W - 44, H - 44), 40, 3, mix(col, (255, 255, 255), 0.35), 200)
    for (x, y) in ((44, 44), (W - 44, 44), (44, H - 44), (W - 44, H - 44)):
        m = Image.new('L', (W, H), 0)
        ImageDraw.Draw(m).polygon([(x - 18, y), (x, y - 18), (x + 18, y), (x, y + 18)], fill=255)
        paint(img, m, gemc)
    # the name banner
    nf = font('Cinzel.ttf', 92 if len(c['name']) <= 9 else 76)
    banner = (150, 74, W - 150, 214)
    paint(img, rr_mask(banner, 24), (4, 6, 14), 220)
    ring(img, banner, 24, 3, col)
    tm = text_mask((W // 2, 92 if len(c['name']) <= 9 else 104), c['name'].upper(), nf)
    glow(img, tm, col, 10, 1.1)
    paint(img, tm, mix(col, (255, 255, 255), 0.55))
    # the tier gems, beside the banner
    for i in range(c['tier']):
        off = (i - (c['tier'] - 1) / 2) * 44
        gem(img, 96, 144 + off, 26, gemc)
        gem(img, W - 96, 144 + off, 26, gemc)
    ef = font('Rajdhani-SemiBold.ttf', 44)
    paint(img, text_mask((W // 2, 228), c['epithet'], ef), mix(col, (230, 236, 245), 0.6))
    # the art, on a halo
    art = Image.open(f"{REPO}/assets/portraits/{c['key']}.png").convert('RGBA').resize((700, 700), Image.LANCZOS)
    halo = Image.new('L', (W, H), 0)
    ImageDraw.Draw(halo).ellipse((W // 2 - 330, 300, W // 2 + 330, 960), fill=200)
    glow(img, halo, col, 70, 0.7)
    img.alpha_composite(art, (W // 2 - 350, 282))
    # the plate
    plate = (78, 1004, W - 78, 1540)
    paint(img, rr_mask(plate, 28), (3, 5, 12), 215)
    ring(img, plate, 28, 3, col, 210)
    lf = font('Rajdhani-Bold.ttf', 30)
    bf = font('Rajdhani-SemiBold.ttf', 46)
    sf = font('Rajdhani-SemiBold.ttf', 38)
    d = ImageDraw.Draw(img)
    x0, y = 118, 1030
    d.text((x0, y), 'DUTY', font=lf, fill=mix(col, (255, 255, 255), 0.3))
    y += 34
    for ln in wrap(c['duty'], bf, W - 2 * x0)[:2]:
        d.text((x0, y), ln, font=bf, fill=(236, 240, 248))
        y += 50
    # the tool classes, held ones lit
    y = max(y + 14, 1180)
    cx, chip_h, cf = x0, 50, font('Rajdhani-Bold.ttf', 30)
    widths = [cf.getsize(k.upper())[0] + 34 for k in CLASSES]
    gap = (W - 2 * x0 - sum(widths)) / (len(CLASSES) - 1)
    for k, wdt in zip(CLASSES, widths):
        box = (int(cx), y, int(cx + wdt), y + chip_h)
        if k in c['powers']:
            paint(img, rr_mask(box, 25), col, 235)
            fill = (8, 10, 18) if sum(col) > 420 else (250, 250, 255)
        else:
            ring(img, box, 25, 2, (90, 98, 120), 200)
            fill = (110, 118, 140)
        d = ImageDraw.Draw(img)
        d.text((box[0] + 17, y + 8), k.upper(), font=cf, fill=fill)
        cx += wdt + gap
    y += chip_h + 26
    for label, value in (('SOIL', c['soil']), c['line']):
        d.text((x0, y + 6), label, font=lf, fill=mix(col, (255, 255, 255), 0.3))
        for i, ln in enumerate(wrap(value, sf, W - 2 * x0 - 190)[:2]):
            d.text((x0 + 190, y + i * 40), ln, font=sf, fill=(208, 214, 228))
        y += 52 if len(wrap(value, sf, W - 2 * x0 - 190)) == 1 else 92
    # the meter: its name and its ok line
    if c['meter']:
        mname, ok = c['meter']
        my = 1460
        d.text((x0, my), 'METER', font=lf, fill=mix(col, (255, 255, 255), 0.3))
        d.text((x0 + 190, my - 6), f'{mname}', font=sf, fill=(208, 214, 228))
        bx0, bx1, by = x0 + 190 + sf.getsize(mname)[0] + 30, W - x0, my + 10
        paint(img, rr_mask((bx0, by, bx1, by + 18), 9), (40, 46, 64))
        okx = bx0 + (bx1 - bx0) * ok // 100
        paint(img, rr_mask((bx0, by, okx, by + 18), 9), col, 230)
        d = ImageDraw.Draw(img)
        d.text((okx - 30, by + 22), f'ok ≥ {ok}', font=font('Rajdhani-SemiBold.ttf', 28), fill=(160, 168, 190))
    # the footer
    ff = font('Cinzel.ttf', 34)
    foot = f"MNEMORPH  ·  {c['kind']}"
    paint(img, text_mask((W // 2, 1580), foot, ff), mix(col, (200, 206, 220), 0.5), 230)
    return img.resize((600, 840), Image.LANCZOS)


def back():
    c = dict(color=(212, 175, 55), gem=(63, 169, 245))
    img = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    body = (20, 20, W - 20, H - 20)
    shape = rr_mask(body, 56)
    grad = Image.new('RGBA', (W, H))
    gd = ImageDraw.Draw(grad)
    for y in range(H):
        gd.line((0, y, W, y), fill=mix((14, 20, 40), (5, 7, 16), y / H) + (255,))
    for i in range(-H, W, 70):
        gd.line((i, 0, i + H, H), fill=(22, 30, 56, 255), width=2)
        gd.line((i + H, 0, i, H), fill=(22, 30, 56, 255), width=2)
    img.alpha_composite(Image.composite(grad, img, shape))
    ring(img, body, 56, 10, c['color'])
    ring(img, (44, 44, W - 44, H - 44), 40, 3, (240, 220, 160), 200)
    halo = Image.new('L', (W, H), 0)
    ImageDraw.Draw(halo).ellipse((W // 2 - 380, 380, W // 2 + 380, 1140), fill=220)
    glow(img, halo, (255, 190, 90), 80, 0.6)
    em = Image.open(f'{REPO}/assets/brand/mnemorph-icon@512.png').convert('RGBA').resize((640, 640), Image.LANCZOS)
    img.alpha_composite(em, (W // 2 - 320, 440))
    tm = text_mask((W // 2, 1180), 'MNEMORPH', font('Cinzel.ttf', 104))
    glow(img, tm, (255, 190, 80), 12, 1.0)
    paint(img, tm, (246, 214, 140))
    for y in (300, 1380):
        gem(img, W // 2, y, 34, c['gem'])
    return img.resize((600, 840), Image.LANCZOS)


if __name__ == '__main__':
    for c in CARDS:
        card(c).save(f"{OUT}/{c['key']}.png", optimize=True)
    back().save(f'{OUT}/back.png', optimize=True)
