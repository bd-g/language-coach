const BIAS: i16 = 0x84;
const CLIP: i16 = 32635;

/// Encodes signed 16-bit PCM samples to 8-bit G.711 mu-law (`PCMU`).
///
/// `PCMU` is lossy; decoded samples are expected to be close to, not identical to, the source.
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#rtp-stream-codec>.
pub fn pcm16_to_mulaw(samples: impl IntoIterator<Item = i16>) -> Vec<u8> {
    samples.into_iter().map(linear_to_mulaw).collect()
}

/// Decodes 8-bit G.711 mu-law (`PCMU`) bytes to signed 16-bit PCM samples.
///
/// `PCMU` is lossy; decoded samples are expected to be close to, not identical to, the source.
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#rtp-stream-codec>.
pub fn mulaw_to_pcm16(bytes: impl IntoIterator<Item = u8>) -> Vec<i16> {
    bytes.into_iter().map(mulaw_to_linear).collect()
}

fn linear_to_mulaw(sample: i16) -> u8 {
    let sign = if sample < 0 { 0x80 } else { 0x00 };
    let clipped = i32::from(sample).abs().min(i32::from(CLIP));
    let biased = clipped + i32::from(BIAS);

    let mut exponent = 7u8;
    let mut exp_mask = 0x4000i32;
    while exponent > 0 && (biased & exp_mask) == 0 {
        exponent -= 1;
        exp_mask >>= 1;
    }

    let mantissa = u8::try_from((biased >> (exponent + 3)) & 0x0f)
        .ok()
        .unwrap_or_default();
    !(sign | (exponent << 4) | mantissa)
}

fn mulaw_to_linear(sample: u8) -> i16 {
    let inv = !sample;
    let sign = inv & 0x80;
    let exponent = (inv >> 4) & 0x07;
    let mantissa = inv & 0x0f;

    let mut value = (i16::from(mantissa) << 3) + BIAS;
    value <<= i16::from(exponent);
    value -= BIAS;
    if sign != 0 { -value } else { value }
}
