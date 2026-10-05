//! The original's three sounds (button press, hit, score reached), straight
//! from Chromium's components/neterror/resources/sounds (BSD-3-Clause; the
//! files are Ogg Vorbis despite their .mp3 names upstream). Decoded once at
//! startup.

use crate::sim::Sound;
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, MixerDeviceSink, Source};
use std::io::Cursor;

const PRESS: &[u8] = include_bytes!("../assets/chromium/button-press.ogg");
const HIT: &[u8] = include_bytes!("../assets/chromium/hit.ogg");
const SCORE: &[u8] = include_bytes!("../assets/chromium/score-reached.ogg");

pub struct Audio {
    // the sink must outlive everything playing on its mixer
    sink: Option<MixerDeviceSink>,
    bank: Vec<SamplesBuffer>,
    pub muted: bool,
}

fn decode(bytes: &'static [u8]) -> Option<SamplesBuffer> {
    match Decoder::new(Cursor::new(bytes)) {
        Ok(d) => {
            let (channels, rate) = (d.channels(), d.sample_rate());
            Some(SamplesBuffer::new(channels, rate, d.collect::<Vec<f32>>()))
        }
        Err(e) => {
            log::warn!("can't decode sound: {e}");
            None
        }
    }
}

impl Audio {
    pub fn new(muted: bool) -> Audio {
        let sink = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut s) => {
                s.log_on_drop(false);
                Some(s)
            }
            Err(e) => {
                log::warn!("no audio: {e}");
                None
            }
        };
        let bank = if sink.is_some() { [PRESS, HIT, SCORE].into_iter().filter_map(decode).collect() } else { Vec::new() };
        Audio { sink, bank, muted }
    }

    pub fn available(&self) -> bool {
        self.sink.is_some() && self.bank.len() == 3
    }

    pub fn play(&self, s: Sound) {
        if self.muted || !self.available() {
            return;
        }
        let Some(sink) = &self.sink else { return };
        let i = match s {
            Sound::Press => 0,
            Sound::Hit => 1,
            Sound::Score => 2,
        };
        sink.mixer().add(self.bank[i].clone());
    }
}
