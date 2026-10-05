# tty-rex

Asciiverse total conversion of Chrome's offline dinosaur game
("No Internet" / the T-Rex runner). Rust + Ratatui + Drawille.

**Status:** Playable (2026-10-04). Own git repo, private at
github.com/theasciiverse/tty-rex (branch `main`). Faithful single-mode recreation: rules,
timings, sprites and sounds from Chromium's own source. See "Built" below for
what exists and the decisions behind it.

## Name

tty-rex — three things collapsed into one word: TTY (the terminal lineage
the whole platform lives in), T-Rex (the actual subject), and the shared
"terminally dad-adjacent" naming register the Asciiverse catalog leans into
deliberately (`av fika` set the tone early; the bar is low on purpose,
and low bars invite better craft, not worse).

## Premise

Chrome's offline "No Internet" dinosaur runner: a T-Rex silhouette running
across a flat ground line, jumping cacti and ducking pterodactyls, endless
and increasingly fast. Deliberately the simplest possible game on the whole
Asciiverse roster — one input axis (jump/duck), one failure state, infinite
runtime. The mechanical simplicity is a feature here, not a limitation: it
means the project is free to focus entirely on architectural cleanliness
rather than fighting complexity.

## The font-first design philosophy — the actual point of this project

Most TUI games adapt themselves to whatever the terminal happens to render.
tty-rex inverts that: **decide the cell geometry and design a purpose-made
font around the game's needs before writing any physics.**

The core problem this solves: terminal cells are rectangular, not square,
and character aspect ratio varies by font and terminal emulator. Obstacle
geometry, jump arc feel, and the ground line all depend on cell aspect
ratio. Deciding cell dimensions first, then designing physics to fit that
known geometry, avoids the usual TUI-game trap of physics that only feels
right by accident on one specific terminal setup.

This is a genuinely novel approach for a TUI game and arguably the real
reason this project exists on the roster — not "port the dino game" so much
as "prove out font-first terminal game design," using the simplest possible
game as the test case before applying the same discipline to something more
complex.

## Rendering approach

- **Drawille (Braille sub-pixel):** the dino sprite itself — roughly 2×4
  dot resolution per cell should be enough for a readable sprite with real
  personality, plus the cactus and pterodactyl silhouettes, which are
  simple enough shapes to survive that resolution cleanly
- **Ratatui (cell grid):** score display, game-over/restart state, any
  minimal menu chrome

## Architecture principle

The mechanical simplicity is explicitly an opportunity to enforce a clean
separation between physics/state, rendering, and input as genuinely
independent layers — Ratatui's model encourages this separation but doesn't
enforce it, and this project should enforce it deliberately rather than
letting the three blur together, since there's no complexity pressure here
forcing shortcuts.

## Built (2026-10-04)

Source of truth for every rule: Chromium's `components/neterror/resources/dino_game`
(BSD-3-Clause), fetched at chromium/chromium `a40910b0aaf2`. The 1x sprite sheet and
the three sounds (`button-press`, `hit`, `score-reached`; Ogg Vorbis despite their
`.mp3` names upstream) are vendored in `assets/chromium/` with the license.

### The geometry decision (font-first, settled)

**One Braille dot = 2x2 original canvas pixels.** On the common 1:2 terminal cell a
Braille cell's 2x4 dots are square, so the 600x150 px canvas is 300x75 dots = 150
columns x 19 rows. Chosen because the 1x sprite sheet is itself pixel art in 2x2
blocks: at this scale every art pixel of the T-Rex, cacti and pterodactyl survives,
and `tools/extract_sprites.py` samples each sprite at the 2x2 phase matching its art
grid (the renderer snaps it to that phase in world space, so the eye never smears).

Physics runs in the original's pixel units and never sees the terminal, so cell
aspect ratio can't change how the game plays, only how square the dots look. That is
the font-first payoff in practice. What is *not* done: an actual purpose-made font.
A terminal renders one font per session, so that belongs to the platform-level
Asciiverse font idea noted in Mato's CLAUDE.md, not to this game.

Scale 1 (added for big windows): one dot per canvas pixel, Chrome's sprites exactly
as drawn, 38 rows. Picked by the same rule as the others (finest that fits with at
least 360 px of road), so from about 180x38; the restart arrow art is doubled
there. Scale 2 is still the home scale for normal windows, not a requirement. For small panes (a split at the bottom of the
screen) the renderer drops to 3..6 px per dot: 13 rows at scale 3, 10 at 4, 7 at 6.
It picks the finest scale whose rows fit and that still shows 360 px of road, else
the finest that fits at all (minimum 240 px road, so 20x7 is the smallest pane).
`--scale N` pins it. Sprites are vendored as raw 1x pixels and downsampled per scale
at startup, each at its best pixel phase. Physics is untouched by any of this, which
is exactly what the font-first split buys.

`--wide` lifts the 600 px cap so the road fills the terminal (opt-in: more
look-ahead than Chrome ever gives, so a little easier). Chrome's ground is two
leapfrogging 600 px strips, which only cover 600 px, so the renderer repeats the
pair across a wider field; obstacles, clouds, stars and the moon already work off
the field width.

The sky shades once per cell row, not per half-row, on purpose: a Braille cell has
one background, so wherever its two halves differed (every half-row of a smooth
gradient) a sprite carried a faint box, only 3-4 levels off but plainly visible
(measured from a real screenshot). Per-row sky bands are the price: visible in
short panes, near invisible at night and in tall windows. Where a Braille cell
straddles a dune edge it takes the half with fewer lit dots, not a blend.

Text over half-block scenery (score, GAME OVER, PAUSED, the look note) is backed
with the plain scenery colour, or a cloud or cactus behind it shows through as a
block (`back_text`; `text_knocks_out_sprites_behind_it` guards it).

A field narrower than 600 px also starts slower, as Chrome's `setSpeed` does
(`width / 600 * 1.2`).

Focus: crossterm focus events. Losing focus pauses (Chrome's `onVisibilityChange`);
unlike Chrome, focus coming back doesn't resume by itself, Space or P does.

### Architecture

Three layers that don't reach into each other:

- `src/sim/`: the `Runner` (offline.ts), `Trex` (trex.ts), `Horizon` (obstacles,
  clouds, ground, night sky). No Ratatui, crossterm or audio. Input arrives as the
  browser's own key-down/key-up calls, time as one 60 Hz frame per `tick()`, and
  sounds/crashes leave as `Event`s. Seedable RNG (`--seed N`) for replays.
- `src/render.rs` + `src/sprites.rs`: picks a `Layout` (scale, columns, rows) for
  the pane, reads `Runner`, paints a layered dot buffer (sky / night / foreground),
  packs it into Braille cells.
- `src/input.rs`: crossterm keys to sim calls. Exact presses/releases with the kitty
  keyboard protocol; elsewhere releases are inferred from key repeats stopping,
  which means every jump is full height there (a tap's release can't be known).

### Hand touches on the T-Rex (`sprites.rs` `TOUCHES`)

Pure downsampling loses the 2x2-pixel eye from scale 3 on (it falls between dots)
and leaves square snout corners. A few anchor points in sheet pixels (eye centre,
snout corners per frame) fix that at every scale where it fits: scales 3 and 4 get a
placed one-dot eye with the head filled in around it, scales 2 to 4 get the snout
corners rounded (only where a dot is a true outside corner). Scale 2 keeps the
sheet's own eyes, which matters for the crash frame's ring-shaped X eye. Past scale
4 the head is about three dots wide and gets no eye, by design.
`cargo test --release show_sprites -- --ignored --nocapture` prints sprites at every
scale (`TTY_REX_SPRITES=a,b` picks them).

### Restart arrow

No button, just a Braille ↻ (`render.rs` `restart_arrow`): hand-drawn pixel art in
three sizes (`ARROW_LARGE` 15x14 dots, about Chrome's 26 px arrow at scale 2;
`ARROW_MEDIUM` 11x10; `ARROW_SMALL` 8x8), all a thick arc with a corner-triangle
head at the top right. It sits centred in the free band between the GAME OVER line
and the ground. Scale 2 uses the large one; coarser scales take the largest that
fits, because small panes need it readable more than they need it to-scale. The
blocks look draws the same art in half-block pixels.

Tried and dropped, in order: Chrome's 8-frame button sprite (the arrow is a 3 px
white cut-out, mush from scale 3 on); a text-cell button with a `↻` glyph (crisp
but the glyph is tiny in real terminals, and the box added nothing); a computed
ring with a computed arrowhead (the head came out as a lump at every size).

### Fidelity notes (things that look like bugs but are the original)

- A jump release before the T-Rex has risen 30 px is ignored, so a very quick tap
  still gives a medium hop.
- The intro slides the dino only while the canvas widens (0.4 s at 1 px/frame), so
  it settles at x ~= 24, not the configured 50.
- Night lasts 12 s from every multiple of 700; the moon starts at x = 0 and wraps.
- Holding jump through a landing jumps again (browser key repeat does the same).

### Colour, half-blocks and Braille (default theme, 2026-10-04)

A deliberate split between the two glyph kinds: scenery behind the action is
half-block colour (two square pixels per cell: sky gradient, two parallax dune
ridges at 1/3 and 1/6 ground speed, pebbled sand below the ground line), and
everything from the original (dino, cacti, pterodactyls, clouds, ground line, moon,
stars) stays Braille, coloured per kind on top. A Braille cell takes the scenery
colour as its background, so there are no holes. The scenery is new; nothing about
the rules changes. The sand's pebbles are locked to the ground via the sim's
`Horizon::scrolled` counter (the one presentation-only field in the sim).

Night blends a day palette into a night one using the night sky's own fade
(`night.opacity`, the moon's fade-in), rather than inverting. Palettes live in
`src/palette.rs`. True colour when `COLORTERM` says so; otherwise 256 colours, where
the scenery uses hand-picked cube entries and a three-band sky (nearest-match of
the blends came out green and grey; checked visually), and the palette snaps
day/night instead of blending.

### Five looks (`render::Look`, `V` cycles, `--look NAME`)

- `braille`: terminal's own colours, night as `REVERSED`, i.e. the original's CSS
  `invert(1)` applied to the user's palette. Clouds are DIM by day only: DIM on a
  reversed cell dims its background, which boxed every cloud at night. Default under `NO_COLOR`/`TERM=dumb`.
- `classic`: the sprite sheet's greys on white, Braille, true colour.
- `color`: coloured Braille on a flat palette "page" (clouds get their own
  `page_cloud` colour; white ones vanished on the pale page).
- `blocks`: everything but the T-Rex is glyph-fitted, chafa-style (`draw_blocks`,
  `fit_cell`): sprites painted at the sheet's own resolution, each cell sampled
  8x8, reduced to two colours (a sprite's colour always kept, so the ground line
  survives between dune and sand) and drawn as the best-matching block glyph
  (eighths, quadrants, halves, `━`, each either way round; no 1/8 or 2/8 wide
  vertical slivers, which poked out of silhouettes as spikes). Cells with real
  sprite presence can't fit to "nothing"; mostly-ground cells take only
  horizontal glyphs; the 1 px ground line is thickened to a quarter cell within
  its own cell row; clouds use `cloud_filled`.
  **The T-Rex is not fitted.** He's drawn after the fit from exactly the
  Braille look's dots (Chrome's pixels at the layout scale, plus the hand
  touches that give him an eye at scales 3 and 4), as solid **block octants**
  (U+1CD00..1CDE5, Unicode 16; 2x4 per cell, i.e. one block pixel per Braille
  dot; `src/octants.rs` maps all 256 patterns, generated from VTE's own
  `minifont.cc` table). At full size that's Chrome's dino pixel for pixel, the
  same as the Braille look but solid. The night stars get the same treatment (with
  octants): a 9 px sparkle came out of the fit as a block and two stray lines,
  which David asked about ("distant birds?"). VTE draws octants itself since 0.78 (no
  font needed; checked in VTE's source), so they're on by default when
  `VTE_VERSION >= 7800` (GNOME Terminal, Ptyxis) or in Ghostty 1.1.1+
  (`TERM_PROGRAM`/`TERM_PROGRAM_VERSION`; its octant table first shipped in
  v1.1.1). kitty (0.40.0, changelog) and foot (1.20.0, changelog) draw them too
  but don't expose their version in the environment, so they need `--octants`;
  WezTerm has them only in nightlies (changelog), Alacritty's builtin font has
  none. All checked in each project's source on 2026-10-05. `--octants` /
  `--no-octants` override. Without octants: quadrants sampled from Chrome's 1x pixels, lit if
  over half dino, with the dino snapped to the alignment that fits his pixel
  grid best; it keeps eye and arm at full size, the silhouette only at 13 rows.
  Quadrants from the Braille dots don't work (two dots per quadrant: "either"
  fills the eye and mouth, "both" drops the thin parts). David's cheat is in
  for the fallback: a cell with real dino in Chrome's pixels but no quadrant
  lit (the arm, the lower jaw, a leg's tip) gets the line or hook character
  (`│ ▏ ▕ ─ ▔ ▁ ╷ ╵ ╴ ╶ ┐ ┘ └ ┌`) whose strokes lie closest to his pixels
  (`fit_lines`, a two-way stroke distance). Kept after a with/without gallery
  comparison: it brings back the jaw line and the arm without clutter.
  `TTY_REX_QUADS=1` makes `dino_gallery` show the fallback. `blocks_dino_is_the_braille_dino_in_octants` checks every dino
  cell against the Braille look's dots, all frames, every x offset, four sizes.
  History, so it isn't repeated: fitting him per frame gave a different face at
  every cell alignment, and patches for eye, mouth and jaw each passed their
  tests while looking worse ("it was better before when his head was just a
  round head"); a hand-drawn half-block version (David: "assign the blocks by
  hand") was the right instinct but half-blocks only hold 2 pixels per cell
  against Braille's 8, so it came out crude ("why do you keep changing from
  that?" with Chrome's frames next to it). The answer was resolution: octants
  are Braille's resolution in solid blocks. Lesson: tests that check a pixel
  exists don't check that a face looks right; look at galleries of every pose.
  David: "the simplest of all the games ... was the one that needed the most
  design care."
- `mixed`: the default, as described above.

All share one layout, one sim and one palette; only the cell-writing differs
(`draw_braille` / `draw_blocks`).

Looking at frames: `TTY_REX_SNAP=<dir> cargo test autopilot` writes day/bird/night/
crash frames in the color, blocks and mixed looks as ANSI text for any
ANSI-to-image tool.

## Open questions

- [x] Exact cell aspect ratio and font design: settled above (2x2 px per dot at
      home scale, coarser for small panes, 1:2 cell assumed, physics independent
      of it). Purpose-made font deferred to
      the platform-level font idea.
- [x] Difficulty/speed curve: faithful (speed 6 to 13, +0.001/frame, Chrome's
      narrow-window scaling).
- [x] Scope: strictly faithful single mode, with day/night, moon phases and stars.
      Not ported: the alt game modes, the slow/a11y audio-cue mode, the restart
      button's newer 8-frame animation (frame 0 only).
- [x] `drawprobe`: refuses to start on Braille rated Unusable/Degraded (or no
      codepoint support), `--force` overrides; no true-colour requirement.
- [x] Sound: the original three clips, played via rodio; `M` mutes.
- [ ] Colour palettes have only been judged in rendered captures, not yet on a
      range of real terminal themes
- [x] Gamepad: Chrome's mapping (offline.ts pollActiveGamepad: button 0 jump,
      1 duck, 9 restart), via gilrs as in Panzer (`gamepad` cargo feature, on
      by default; SDL-database pads only, `TTY_REX_ANY_PAD=1` for any). Start
      also pauses when not crashed; the D-pad mirrors jump and duck. Built and
      wired; not tried with a physical pad in this session.
- [x] Octants beyond VTE: researched per terminal (see the blocks look notes);
      Ghostty auto-detected, kitty and foot documented for `--octants`.
- [x] Resize mid-run: checked in offline.ts `adjustDimensions`: Chrome doesn't
      recompute the speed on resize (only at start and restart), it stops the
      game. tty-rex now pauses on resize too (an earlier note here claimed
      Chrome re-ran `setSpeed`; it doesn't).

## Ideas for later (David, 2026-10-04: "later, not now")

- **Attract / ambient mode:** a self-playing strip for the bottom of a larger tmux
  layout, "almost like a little active entertainment". Groundwork already exists:
  the sim is input-agnostic (an autopilot just calls `jump_down`/`duck_down`),
  `render.rs` tests already contain a simple autopilot (needs to get smarter about
  high pterodactyls to run for long), `--wide` and the scale-6 layout already fit
  a 20x7 strip, and focus auto-pause would need to be off in this mode.
- **Show the running system in it, ncmatrix-style:** something of the live
  machine woven into the scene (e.g. load or network activity driving the clouds,
  stars or obstacle rhythm, or text/data rain in the sky). Per David, ncmatrix
  highlights raining glyphs from network traffic; that's the reference point.
  Not designed yet.

## Commands

```bash
cargo run --release                 # play
cargo test                          # sim + render tests
cargo test autopilot -- --nocapture # prints frames with birds, night, a crash
python3 tools/extract_sprites.py    # rebuild assets/sprites.txt (needs Pillow)
cargo run --release -- --scale 4    # try a coarser scale in a big window
RUST_LOG=debug cargo run --release 2>tty-rex.log
```
