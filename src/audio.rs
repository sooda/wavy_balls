use std::marker::PhantomData;
use std::path::Path;

use sdl2::mixer::{init, INIT_OGG, Sdl2MixerContext, open_audio, AUDIO_S16LSB, allocate_channels,
                  Chunk, Channel, EffectCallback, Music};
use ::sdl_err;

use errors::*;

// One sdl mixer chunk has a volume attribute, but it's global for that chunk and the chunk is
// loaded from a file, so multiple simultaneous similar clips with different volume would need to
// be loaded separately. We use the effect system to lower the volume then. More complex effects
// are anticipated too, so this makes sense.

struct SoundClip {
    chunk: Chunk,
}

impl SoundClip {
    fn new(filename: &str) -> Result<Self> {
        let chunk = Chunk::from_file(Path::new(filename)).map_err(sdl_err)
            .chain_err(|| "failed to load jump sound")?;
        Ok(SoundClip { chunk: chunk })
    }
}

trait StereoFilter: Send {
    // takes interleaved left, right samples
    fn filter(&mut self, &mut [i16]);
}

struct AudioTape<'a, F: StereoFilter> {
    clip: &'a SoundClip,
    filter: F,
}

struct SdlCallback<F: StereoFilter> {
    filter: F,
}

impl<F: StereoFilter> EffectCallback for SdlCallback<F> {
    type SampleType = i16; // this matches AUDIO_S16LSB for open_audio

    fn callback(&mut self, buf: &mut [i16]) {
        self.filter.filter(buf);
    }
}

pub struct AudioMixer<'a> {
    phantom_clip: PhantomData<&'a SoundClip>,
    sdl_mixer: Sdl2MixerContext,
    music: Music,
}

impl<'a> AudioMixer<'a> {
    pub fn new(music_filename: &str) -> Result<Self> {
        let sdl_mixer = init(INIT_OGG).map_err(sdl_err)
            .chain_err(|| "failed to initialize SDL mixer")?;

        open_audio(44100, AUDIO_S16LSB, 2, 2048).map_err(sdl_err)
            .chain_err(|| "failed to open SDL audio")?;
        allocate_channels(16);

        let music =
            Music::from_file(Path::new("foldplop_-_memory_song_part_2.ogg")).map_err(sdl_err)
                .chain_err(|| "failed to load background music")?;

        music.play(-1).map_err(sdl_err).chain_err(|| "failed to play background music")?;

        Ok(AudioMixer {
            phantom_clip: PhantomData,
            sdl_mixer: sdl_mixer,
            music: music,
        })
    }

    pub fn play<F: StereoFilter>(&self, tape: AudioTape<'a, F>) -> Result<()> {
        let chan = Channel::all().play(&tape.clip.chunk, 0)
            .map_err(sdl_err)
            .chain_err(|| "failed to play sound")?;

        // btw, SDL2_mixer removes all effects from a channel when the channel
        // is done playing, and that's when the effect is dropped, if not
        // earlier explicitly
        chan.register_effect(SdlCallback { filter: tape.filter })
            .map_err(sdl_err)
            .chain_err(|| "failed to effect")?;

        Ok(())
    }
}

struct VolumeEffect {
    vol: f32,
}

impl StereoFilter for VolumeEffect {
    fn filter(&mut self, buf: &mut [i16]) {
        for i in buf.iter_mut() {
            *i = ((*i as f32) * self.vol) as i16;
        }
    }
}

pub struct JumpSound {
    clip: SoundClip,
}

impl JumpSound {
    pub fn new() -> Result<Self> {
        Ok(JumpSound { clip: SoundClip::new("146718__fins__button.wav")? })
    }
    pub fn play(&self, vol: f32) -> AudioTape<VolumeEffect> {
        AudioTape {
            clip: &self.clip,
            filter: VolumeEffect { vol: vol },
        }
    }
}
