//! tty-rex is drawn in Braille dots, so it refuses to start where drawprobe
//! says Braille renders badly, rather than showing an illegible smudge.
//! `--force` skips the check. No true-colour requirement: the default theme
//! uses the terminal's own colours.

use drawprobe::RenderQuality;

pub fn check() -> Result<(), String> {
    let braille = drawprobe::assess().braille;
    log::info!("drawprobe braille: support {} quality {:?} confidence {:?}", braille.codepoint_support, braille.render_quality, braille.confidence);
    if !braille.codepoint_support || matches!(braille.render_quality, RenderQuality::Unusable | RenderQuality::Degraded) {
        return Err(format!(
            "tty-rex draws in Braille dots, which this terminal doesn't render well \
             (render_quality: {:?}, confidence: {:?}). Run with --force to try anyway.",
            braille.render_quality, braille.confidence
        ));
    }
    Ok(())
}
