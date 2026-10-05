# tty-rex

Chrome's offline "No Internet" T-Rex runner, in the terminal, drawn in Braille dots.
Part of the [Asciiverse](https://github.com/theasciiverse) catalog.

```
cargo run --release
```

Space or Up to start. The canvas opens up after the first jump lands, exactly as in
Chrome, then it's cacti, pterodactyls past speed 8.5, and night every 700 points.

Runs in anything from a 20x7 split pane up to a big window: the Braille dots get
coarser or finer to fit (`--scale 1` is Chrome's own pixels at 38 rows, 6 the
coarsest), the game itself doesn't change. A full 600 px road takes 150 columns at the finest scale, 75 at scale 4; a
narrower pane sees less road and starts slower, as Chrome does in a narrow window.
For ultra-wide play, `--wide` runs the road the full terminal width instead of stopping
at Chrome's 600 px (you see further ahead, so it's a little easier).
Switching away from the terminal pauses the game (where the terminal reports focus).

Braille has to render well; that's checked at launch with `drawprobe` (`--force`
skips the check).

## Keys

| Key | |
|---|---|
| Space, Up, W, K | jump (hold for a higher jump) |
| Down, S, J | duck; mid-air, drop fast |
| Enter | restart after a crash (Space works too, after a moment) |
| P | pause (Space resumes too) |
| V | next look |
| A | auto mode: he runs on his own (any jump or duck key takes over) |
| M | mute |
| Q, Esc | quit |

A gamepad works as in Chrome: A jumps, B ducks, Start restarts after a crash (and
pauses otherwise); the D-pad's up and down jump and duck too, and Select toggles auto mode.

## Auto mode

`A` in game, or `tty-rex --auto`, lets him run on his own: a strip of ambient
entertainment for, say, the bottom pane of a tmux layout. He plays well but not
perfectly (now and then he's a moment late, more often at speed), so runs last a few
minutes; after a crash he gets back up within 3 seconds. The sound drops to 40%, he
doesn't pause when you switch panes or resize, and his scores don't touch your high
score. For a strip: `tty-rex --auto --wide` in a short pane.

Variable jump height needs key release events, which terminals speaking the kitty
keyboard protocol send (kitty itself, for one). Elsewhere a release can only be guessed
from the key repeats stopping, so every jump is a full one.

## Looks

`V` cycles through them in game and remembers the choice for next time (in
`~/.config/tty-rex/look`); `--look NAME` picks one for a single run.

| Look | |
|---|---|
| `braille` | black and white Braille in the terminal's own colours; night inverts them, like Chrome |
| `classic` | Braille in Chrome's grey on white (needs true colour) |
| `color` | coloured Braille sprites on a plain page |
| `blocks` | everything in colour block glyphs, each cell fitted to the original pixels; solid pixel-art clouds; the T-Rex in block octants, as sharp as Braille, else quadrants |

Block octants are new (Unicode 16) and only some terminals draw them. They're used
automatically in VTE 0.78+ (GNOME Terminal, Ptyxis and others) and Ghostty 1.1.1+.
kitty 0.40+ and foot 1.20+ draw them too but don't announce their version, so pass
`--octants` there. WezTerm has them only in nightly builds; Alacritty doesn't.
| `mixed` | the default: half-block scenery (sky, parallax dunes, pebbled sand) behind coloured Braille sprites |

In colour: Chrome's grey dino (red when it crashes), green cacti, violet pterodactyls, an
amber score, and a night that fades into a dusk palette with a pale moon. True colour
where the terminal says so (`COLORTERM=truecolor`), 256 colours otherwise. `NO_COLOR`
makes `braille` the default.

## Options

`tty-rex --help`: `--look NAME`, `--wide`, `--octants`/`--no-octants`, `--mute`, `--force`, `--scale N`, `--seed N`
(`--mono` and `--classic` are short for `--look braille` and `--look classic`).

The high score is kept in `$XDG_DATA_HOME/tty-rex/highscore` (`~/.local/share/...`).

Session log for bug reports: `RUST_LOG=debug tty-rex 2>tty-rex.log`

## Credits

Rules, timings, sprites and sounds come from Chromium's dino game
(`components/neterror/resources/dino_game`, sprite sheet and sounds alongside it),
BSD-3-Clause, copyright The Chromium Authors; see `assets/chromium/LICENSE`.
`tools/extract_sprites.py` rebuilds `assets/sprites.txt` from the sprite sheet.
