# Roadmap

tty-rex is at 1.0.0 (2026-10-28): the faithful single-mode recreation of
Chrome's offline T-Rex game is complete.

## 1.0.0: built

- [x] Chromium's rules, timings, speed curve, sprites and its three sounds
- [x] Day and night, moon phases and stars
- [x] Braille graphics at several scales, and the blocks look
- [x] Keyboard and gamepad (Chrome's mapping)
- [x] Auto mode: he runs on his own, for a strip in a tmux split

## 1.0.1: built

- [x] Gamepads work on Windows: gilrs reads them through XInput, since its
  default (Windows Gaming Input) only reports to a program with a focused
  window of its own, which a terminal game never has

## 1.0.x

- [ ] Try the gamepad with a physical pad (built and wired, not yet tried)

## After 1.0

- [ ] The running system in the scene, ncmatrix-style: load or network
  activity driving the clouds, stars or obstacle rhythm, or data rain in
  the sky. Not designed yet ("later, not now", David, 2026-10-04)
