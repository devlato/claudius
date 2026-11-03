//! A sampled loop as the demo's sampler keeps it: decoded from the Ogg the free libraries send, at the demo's rate,
//! with its loudness.

/// the rate the demo's audio is at
pub const RATE: u32 = 48_000;

/// a decoded file, at the demo's rate; `r` is empty for a mono one
pub struct Clip {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    pub secs: f32,
    /// its K-weighted power while it plays
    pub power: f32,
}

/// an Ogg Vorbis file as samples, and the rate they are at
pub fn decode_at(bytes: Vec<u8>) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    use symphonia::core::codecs::CodecParameters;
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::errors::Error as Sym;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("ogg");
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| e.to_string())?;
    let track = format.default_track(TrackType::Audio).ok_or("no audio in it")?;
    let id = track.id;
    let Some(CodecParameters::Audio(params)) = track.codec_params.clone() else { return Err("no codec in it".into()) };
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| e.to_string())?;
    let channels = params.channels.as_ref().map_or(1, |c| c.count()).max(1);
    let rate = params.sample_rate.unwrap_or(RATE);
    let (mut l, mut r, mut buf) = (Vec::new(), Vec::new(), Vec::new());
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) | Err(Sym::ResetRequired) => break,
            Err(Sym::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id != id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(audio) => {
                audio.copy_to_vec_interleaved(&mut buf);
                for frame in buf.chunks_exact(channels) {
                    l.push(frame[0]);
                    if channels > 1 {
                        r.push(frame[1]);
                    }
                }
            }
            Err(Sym::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok((l, r, rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// one of claudius' own sounds, its pluck preset at F#4
    const PLUCK: &[u8] = include_bytes!("testdata/pluck-66.ogg");

    #[test]
    fn an_ogg_file_decodes() {
        let (l, r, rate) = decode_at(PLUCK.to_vec()).unwrap();
        assert!(l.len() > 20_000 && l.len() == r.len() && rate == RATE, "{} {} {rate}", l.len(), r.len());
        assert!(l.iter().fold(0f32, |m, x| m.max(x.abs())) > 0.3);
    }
}
