#!/usr/bin/env python3
"""Regenerate the .theme files beside this script (T012, T013).

NOT part of the build. The .theme files are the artifact and are committed;
this exists so the DERIVED values in them can be audited and reproduced rather
than taken on trust. Every documented value is a literal here (quoted from
Design Language, mockup 8c/9a, or a published Catppuccin / Tokyo Night role);
every derived value is the phosphor-dark relationship re-applied to the target
palette, which is the method each .theme file's header describes.

    OUT=crates/phosphor-ui/themes python3 crates/phosphor-ui/themes/derive.py

It prints the actor-hue validation table as it goes, so a palette that would
fail Theme::load is visible before Rust ever sees it. Changing a value here
changes nothing until you re-run it AND the Rust tests still pass — the
phosphor-dark file is asserted equal to Theme::phosphor_dark() field for field.
"""
import colorsys, os

def hx(s):
    s = s.lstrip('#')
    return tuple(int(s[i:i+2], 16) for i in (0, 2, 4))

def to_hex(rgb):
    return '#%02x%02x%02x' % tuple(max(0, min(255, int(round(c)))) for c in rgb)

def hsl(s):
    r, g, b = [c/255 for c in hx(s)]
    h, l, sat = colorsys.rgb_to_hls(r, g, b)
    return h*360, sat, l

def from_hsl(h, s, l):
    r, g, b = colorsys.hls_to_rgb((h % 360)/360, max(0, min(1, l)), max(0, min(1, s)))
    return to_hex((r*255, g*255, b*255))

def hue(s):
    return hsl(s)[0]

def chroma(s):
    r, g, b = hx(s)
    return (max(r, g, b) - min(r, g, b)) / 255

# ── the four palettes ────────────────────────────────────────────────────
P = {}

P['phosphor-dark'] = dict(
    name='phosphor', variant='dark',
    claude='#3ddc97', you='#82aecd', attention='#e0a94e',
    trouble='#d97b6c', transient='#cfa86a', steel='#9ec98c',
    ground='#0c0f0c', text='#c6cec6', prose='#9aa39a', meta='#59635a',
    line_numbers='#414b42', dimmed='#232823', bright_text='#e8f0e8',
    anchor='#141d16', anchor_undercurl='#2a5c44', selection='#26332a',
    failure='#211114',
    fl_info='#2a5c44', fl_need='#6b5426', fl_need_body='#171207',
    fl_need_rule='#3d3418', fl_passive='#2a3c2e', fl_body='#101410',
    statusline='#1a201a', tab_bar_rule='#1d241d', divider='#242a24',
    keyword='#82aecd',
)

P['phosphor-light'] = dict(
    name='phosphor', variant='light',
    claude='#1a9a62', you='#3568a8', attention='#a06a10',
    trouble='#b5473a', transient='#8f6a2e', steel=None,
    ground='#f4f2ec', text='#3a3830', prose=None, meta='#8a8474',
    line_numbers='#b0aa98', dimmed=None, bright_text=None,
    anchor='#e6efe6', anchor_undercurl='#1a9a62', selection=None,
    failure=None,
    fl_info=None, fl_need=None, fl_need_body=None,
    fl_need_rule=None, fl_passive=None, fl_body=None,
    statusline='#e8e4d8', tab_bar_rule=None, divider='#d8d4c8',
    keyword='#3568a8',
)

P['catppuccin-mocha'] = dict(
    name='catppuccin', variant='dark',
    claude='#a6e3a1', you='#89b4fa', attention='#f9e2af',
    trouble='#f38ba8', transient='#fab387', steel='#94e2d5',
    ground='#1e1e2e', text='#cdd6f4', prose='#a6adc8', meta='#6c7086',
    line_numbers='#45475a', dimmed=None, bright_text=None,
    anchor='#232336', anchor_undercurl='#a6e3a1', selection=None,
    failure=None,
    fl_info=None, fl_need=None, fl_need_body=None,
    fl_need_rule=None, fl_passive=None, fl_body=None,
    statusline='#181825', tab_bar_rule=None, divider='#313244',
    keyword='#cba6f7',
)

P['catppuccin-latte'] = dict(
    name='catppuccin', variant='light',
    claude='#40a02b', you='#1e66f5', attention='#df8e1d',
    trouble='#d20f39', transient='#fe640b', steel='#179299',
    ground='#eff1f5', text='#4c4f69', prose='#6c6f85', meta='#8c8fa1',
    line_numbers='#bcc0cc', dimmed=None, bright_text=None,
    anchor='#e2e6ec', anchor_undercurl='#40a02b', selection=None,
    failure=None,
    fl_info=None, fl_need=None, fl_need_body=None,
    fl_need_rule=None, fl_passive=None, fl_body=None,
    statusline='#e6e9ef', tab_bar_rule=None, divider='#ccd0da',
    keyword='#8839ef',
)

P['tokyo-night'] = dict(
    name='tokyo night', variant='dark',
    claude='#9ece6a', you='#7aa2f7', attention='#e0af68',
    trouble='#f7768e', transient='#ff9e64', steel='#73daca',
    ground='#1a1b26', text='#c0caf5', prose='#a9b1d6', meta='#565f89',
    line_numbers='#3b4261', dimmed='#292e42', bright_text=None,
    anchor=None, anchor_undercurl='#9ece6a', selection=None,
    failure=None,
    fl_info=None, fl_need=None, fl_need_body=None,
    fl_need_rule=None, fl_passive=None, fl_body=None,
    statusline='#16161e', tab_bar_rule=None, divider='#292e42',
    keyword='#bb9af7',
)

P['tokyo-night-day'] = dict(
    name='tokyo night', variant='light',
    claude='#587539', you='#2e7de9', attention='#8c6c3e',
    trouble='#f52a65', transient='#b15c00', steel='#118c74',
    ground='#e1e2e7', text='#3760bf', prose='#6172b0', meta='#848cb5',
    line_numbers='#a8aecb', dimmed='#c4c8da', bright_text=None,
    anchor=None, anchor_undercurl='#587539', selection=None,
    failure=None,
    fl_info=None, fl_need=None, fl_need_body=None,
    fl_need_rule=None, fl_passive=None, fl_body=None,
    statusline='#d0d5e3', tab_bar_rule=None, divider='#c4c8da',
    keyword='#9854f1',
)

# ── derivations, all measured off the phosphor-dark reference ────────────
# lightness deltas from ground, measured on phosphor dark:
D_ANCHOR, D_DIMMED, D_SELECT, D_FAILURE, D_STATUS, D_TABRULE, D_BODY = (
    0.043, 0.094, 0.122, 0.047, 0.061, 0.074, 0.018)
# ratios measured off phosphor dark's actor -> chrome pairs:
INFO_S, INFO_L = 0.54, 0.48        # claude    -> float.informational
NEED_S, NEED_L = 0.60, 0.48        # attention -> float.needs_you
RULE_L = 0.59                      # needs_you -> needs_you_rule
PASSIVE_S, PASSIVE_L = 0.25, 0.20  # claude    -> float.passive (abs L on dark)
BRIGHT_D = 0.135                   # text -> bright_text, away from ground


def derive(p):
    dark = p['variant'] == 'dark'
    sign = 1 if dark else -1
    gh, gs, gl = hsl(p['ground'])
    th, ts, tl = hsl(p['text'])
    ch, cs, cl = hsl(p['claude'])
    ah, as_, al = hsl(p['attention'])
    rh, rs, rl = hsl(p['trouble'])

    def g(delta, s_bump=0.02):
        return from_hsl(gh, gs + s_bump, gl + sign * delta)

    out = dict(p)
    if out['steel'] is None:                       # phosphor light only
        sh, ss, sl = hsl(P['phosphor-dark']['steel'])
        out['steel'] = from_hsl(sh, ss * 1.023, sl * 0.641)
    if out['prose'] is None:                       # phosphor light only
        # sits 40.5% of the way from text to meta, as it does on phosphor dark
        mh, ms, ml = hsl(p['meta'])
        out['prose'] = from_hsl((th + mh) / 2, (ts + ms) / 2, tl + 0.405 * (ml - tl))
    if out['bright_text'] is None:
        # never all the way to #ffffff / #000000: "bright text" is the top of the
        # palette's own ramp, not the absence of one.
        out['bright_text'] = from_hsl(th, ts, max(0.06, min(0.94, tl + sign * BRIGHT_D)))
    if out['anchor'] is None:
        out['anchor'] = g(D_ANCHOR)
    if out['dimmed'] is None:
        out['dimmed'] = g(D_DIMMED)
    if out['selection'] is None:
        out['selection'] = g(D_SELECT)
    if out['failure'] is None:
        out['failure'] = from_hsl(rh, 0.31, gl + sign * D_FAILURE)
    if out['statusline'] is None:
        out['statusline'] = g(D_STATUS)
    if out['tab_bar_rule'] is None:
        out['tab_bar_rule'] = g(D_TABRULE)
    if out['divider'] is None:
        out['divider'] = g(D_DIMMED)
    if out['fl_info'] is None:
        out['fl_info'] = (from_hsl(ch, cs * INFO_S, cl * INFO_L) if dark
                          else from_hsl(ch, cs, cl * 0.80))
    if out['fl_need'] is None:
        out['fl_need'] = (from_hsl(ah, as_ * NEED_S, al * NEED_L) if dark
                          else from_hsl(ah, as_, al * 0.80))
    if out['fl_need_body'] is None:
        out['fl_need_body'] = (from_hsl(ah, 0.53, gl + 0.006) if dark
                               else from_hsl(ah, 0.62, gl - 0.030))
    if out['fl_need_rule'] is None:
        nh, ns, nl = hsl(out['fl_need'])
        out['fl_need_rule'] = (from_hsl(nh, ns * 0.91, nl * RULE_L) if dark
                               else from_hsl(nh, ns * 0.55, nl + 0.34))
    if out['fl_passive'] is None:
        out['fl_passive'] = (from_hsl(ch, PASSIVE_S, PASSIVE_L) if dark
                             else from_hsl(ch, 0.22, gl - 0.16))
    if out['fl_body'] is None:
        out['fl_body'] = g(D_BODY, 0.0)
    return out


# ── families, and the arcs each actor is locked to ───────────────────────
FAMILIES = {
    'claude':    ('green',      70.0, 175.0),
    'you':       ('blue',      195.0, 260.0),
    'attention': ('amber',      18.0,  65.0),
    'trouble':   ('red',       335.0,  18.0),
    'transient': ('amber',      18.0,  65.0),
    'steel':     ('green-teal', 70.0, 195.0),
}


def in_arc(h, lo, hi):
    return (lo <= h < hi) if lo < hi else (h >= lo or h < hi)


TEMPLATE = """\
# {title}
#
# base16-style: one `key: value` per line, values are `#rrggbb`.
# A line whose first non-blank character is `#` is a comment; there are no
# trailing comments (a value is the rest of its line).
#
# `actor.*` is the contract, not a preference: each actor is locked to a hue
# family and `Theme::load` rejects a file that moves one out of it. Hue is
# fixed; saturation and lightness are yours (Design Language §10).
#
{prov}
name: {name}
variant: {variant}

# actors — Design Language §1. Validated at load.
actor.claude: {claude}
actor.you: {you}
actor.attention: {attention}
actor.trouble: {trouble}
actor.transient: {transient}
actor.steel: {steel}

# neutral ramp — §1
neutral.ground: {ground}
neutral.text: {text}
neutral.prose: {prose}
neutral.meta: {meta}
neutral.line_numbers: {line_numbers}
neutral.dimmed_under_float: {dimmed}
neutral.bright_text: {bright_text}

# region tints and undercurl — §3
region.anchor: {anchor}
region.anchor_undercurl: {anchor_undercurl}
region.selection: {selection}
region.failure: {failure}
region.failure_undercurl: {trouble}

# float chrome — §4
float.informational: {fl_info}
float.needs_you: {fl_need}
float.needs_you_body: {fl_need_body}
float.needs_you_rule: {fl_need_rule}
float.passive: {fl_passive}
float.body: {fl_body}

# statusline and tab bar — §5
chrome.statusline: {statusline}
chrome.mode_chip_fg: {ground}
chrome.tab_bar: {ground}
chrome.tab_bar_rule: {tab_bar_rule}
chrome.divider: {divider}

# syntax — theme-owned (§10). Not validated.
syntax.text: {text}
syntax.keyword: {keyword}
syntax.type: {transient}
syntax.function: {steel}
syntax.constant: {transient}
syntax.string: {transient}
syntax.number: {transient}
syntax.comment: {meta}
"""

TITLES = {
    'phosphor-dark': 'phosphor · dark — the v1 default (Design Language §1, §10)',
    'phosphor-light': 'phosphor · light — warm paper with deepened hues (§10, mockup 8c)',
    'catppuccin-mocha': 'catppuccin · mocha — mapping (mockup 9a, left)',
    'catppuccin-latte': 'catppuccin · latte — mapping (mockup 9a, right)',
    'tokyo-night': 'tokyo night · night — mapping (Q7; no mockup, inherits 9b\'s shape)',
    'tokyo-night-day': 'tokyo night · day — mapping (Q7; no mockup, inherits 9b\'s shape)',
}

PROV = {
 'phosphor-dark': '''# PROVENANCE — every value is Design Language §1/§3/§4/§5, transcribed. This
# file and `Theme::phosphor_dark()` are two encodings of one palette and a test
# asserts they agree field for field; edit one and the other fails the build.''',
 'phosphor-light': '''# PROVENANCE — mockup `8c` (and `9c` right) give: claude, you, attention,
# trouble, transient, ground, text, line_numbers, meta, region.anchor,
# region.anchor_undercurl, chrome.statusline, chrome.mode_chip_fg (= ground),
# chrome.divider. §10 pins claude-green at #1a9a62.
# DERIVED, because no doc or mockup carries a light value for them: steel,
# prose, dimmed_under_float, bright_text, region.selection, region.failure,
# all six float.*, chrome.tab_bar_rule. Method below.''',
 'catppuccin-mocha': '''# PROVENANCE — mockup `9a` (left) gives: claude=green, attention=yellow,
# transient=peach, ground=base, text=text, meta=overlay0, line_numbers=surface1,
# region.anchor, region.anchor_undercurl, chrome.statusline=mantle,
# chrome.mode_chip_fg=base, chrome.divider=surface0, syntax.keyword=mauve.
# The rest map phosphor roles onto published Catppuccin roles: you=blue,
# trouble=red, steel=teal, prose=subtext0.
# DERIVED: dimmed_under_float, bright_text, region.selection, region.failure,
# all six float.*, chrome.tab_bar_rule. Method below.''',
 'catppuccin-latte': '''# PROVENANCE — mockup `9a` (right), same role mapping as mocha against the
# published Latte palette.
# DERIVED: dimmed_under_float, bright_text, region.selection, region.failure,
# all six float.*, chrome.tab_bar_rule. Method below.''',
 'tokyo-night': '''# PROVENANCE — Q7 replaced Ayu with Tokyo Night; there is no mockup, so `9b`
# stands as the acceptance *shape* only (same slice of UI, second palette, actor
# contract intact). Published Tokyo Night roles: claude=green, you=blue,
# attention=yellow, trouble=red, transient=orange, steel=green1,
# ground=bg, text=fg, prose=fg_dark, meta=comment, line_numbers=fg_gutter,
# dimmed_under_float=bg_highlight, chrome.statusline=bg_dark,
# chrome.divider=bg_highlight, syntax.keyword=magenta.
# DERIVED: bright_text, region.anchor, region.selection, region.failure,
# all six float.*, chrome.tab_bar_rule. Method below.''',
 'tokyo-night-day': '''# PROVENANCE — as tokyo-night, against the published Tokyo Night Day palette
# (a real light variant, which is half of why Q7 chose it).
# DERIVED: bright_text, region.anchor, region.selection, region.failure,
# all six float.*, chrome.tab_bar_rule. Method below.''',
}

METHOD = '''#
# DERIVATION METHOD — a derived value is never invented freehand. Each is the
# phosphor-dark relationship re-applied to this palette: a lightness offset from
# this theme's own `ground` (anchor +/-0.043, dimmed +/-0.094, selection
# +/-0.122, failure +/-0.047, statusline +/-0.061, tab_bar_rule +/-0.074,
# float.body +/-0.018 in HSL L, sign by variant), or a ratio off this theme's
# own actor colour (float.informational = claude at S x0.54 / L x0.48 on dark;
# float.needs_you = attention likewise; region.failure takes trouble's hue).
# Reproduce or re-derive them and the numbers come back.'''

out_dir = os.environ.get('OUT', '/tmp/themes')
os.makedirs(out_dir, exist_ok=True)

print(f'{"theme":18} {"actor":10} {"hex":9} {"hue":>7}  family        chroma')
for slug, base in P.items():
    d = derive(base)
    for actor, (fam, lo, hi) in FAMILIES.items():
        v = d[actor]
        h, c = hue(v), chroma(v)
        ok = in_arc(h, lo, hi) and c >= 0.12
        print(f'{slug:18} {actor:10} {v:9} {h:7.1f}  {fam:12} {c:.3f} '
              f'{"OK" if ok else "**FAIL**"}')
    vals = [d[a] for a in FAMILIES]
    if len(set(vals)) != 6:
        print(f'  !! {slug}: actor colours are not pairwise distinct')
    prov = PROV[slug]
    with open(os.path.join(out_dir, slug + '.theme'), 'w') as f:
        f.write(TEMPLATE.format(title=TITLES[slug], prov=prov + METHOD, **d))
print('wrote', out_dir)
