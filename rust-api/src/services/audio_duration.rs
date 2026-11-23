// An audio file's length from its headers alone. Reading and parsing every MP3 frame (what the
// mp3-metadata crate does) means reading the whole file, which for a large local library is
// tens of gigabytes. Formats: MP3 (Xing/Info/VBRI header, or the bitrate for constant-bitrate
// files), MP4/M4A, FLAC, WAV, Ogg Vorbis/Opus.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Bytes read from the start of an MP3 looking for its first frame (after the ID3 tag).
const MP3_SEARCH: usize = 256 * 1024;
/// Bytes read from the end of an Ogg file looking for its last page.
const OGG_TAIL: u64 = 64 * 1024;

/// The file's length in whole seconds, or None when the format isn't recognised or the
/// headers don't say.
pub fn audio_duration(path: &Path) -> Option<i32> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut head = [0u8; 12];
    let n = read_up_to(&mut file, &mut head)?;
    let head = &head[..n];
    let seconds = if head.starts_with(b"fLaC") {
        flac(&mut file)
    } else if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WAVE") {
        wav(&mut file, len)
    } else if head.starts_with(b"OggS") {
        ogg(&mut file, len)
    } else if head.get(4..8) == Some(b"ftyp") {
        mp4(&mut file, len)
    } else {
        mp3(&mut file, len)
    }?;
    (seconds.is_finite() && seconds > 0.0 && seconds < i32::MAX as f64).then(|| seconds.round() as i32)
}

fn read_up_to(file: &mut File, buf: &mut [u8]) -> Option<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => return None,
        }
    }
    Some(filled)
}

fn read_at(file: &mut File, offset: u64, buf: &mut [u8]) -> Option<usize> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    read_up_to(file, buf)
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn be64(b: &[u8]) -> u64 {
    u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

// ---- MP3 ------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct FrameHeader {
    mpeg1: bool,
    layer: u8,
    bitrate: u32,
    sample_rate: u32,
    mono: bool,
    length: usize,
    samples: u32,
}

fn frame_header(b: &[u8]) -> Option<FrameHeader> {
    if b.len() < 4 || b[0] != 0xFF || b[1] & 0xE0 != 0xE0 {
        return None;
    }
    let version = (b[1] >> 3) & 3; // 3 = MPEG1, 2 = MPEG2, 0 = MPEG2.5
    let layer = match (b[1] >> 1) & 3 {
        3 => 1,
        2 => 2,
        1 => 3,
        _ => return None,
    };
    if version == 1 {
        return None;
    }
    let mpeg1 = version == 3;
    let bitrate_index = (b[2] >> 4) as usize;
    let rate_index = ((b[2] >> 2) & 3) as usize;
    if bitrate_index == 0 || bitrate_index == 15 || rate_index == 3 {
        return None;
    }
    const V1: [[u32; 14]; 3] = [
        [32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448],
        [32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384],
        [32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320],
    ];
    const V2: [[u32; 14]; 2] = [
        [32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256],
        [8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
    ];
    let kbps = if mpeg1 { V1[layer as usize - 1][bitrate_index - 1] } else { V2[(layer != 1) as usize][bitrate_index - 1] };
    let base = [44100, 48000, 32000][rate_index];
    let sample_rate = match version {
        3 => base,
        2 => base / 2,
        _ => base / 4,
    };
    let samples = match (layer, mpeg1) {
        (1, _) => 384,
        (2, _) | (3, true) => 1152,
        _ => 576,
    };
    let padding = ((b[2] >> 1) & 1) as usize;
    let bitrate = kbps * 1000;
    let length = if layer == 1 {
        (12 * bitrate as usize / sample_rate as usize + padding) * 4
    } else {
        samples as usize / 8 * bitrate as usize / sample_rate as usize + padding
    };
    Some(FrameHeader { mpeg1, layer, bitrate, sample_rate, mono: b[3] >> 6 == 3, length, samples })
}

/// Where the audio starts: past an ID3v2 tag, if there is one.
fn id3v2_end(file: &mut File) -> Option<u64> {
    let mut h = [0u8; 10];
    if read_at(file, 0, &mut h)? < 10 || &h[..3] != b"ID3" {
        return Some(0);
    }
    let size = h[6..10].iter().fold(0u64, |acc, b| (acc << 7) | u64::from(b & 0x7F));
    let footer = if h[5] & 0x10 != 0 { 10 } else { 0 };
    Some(10 + size + footer)
}

fn mp3(file: &mut File, len: u64) -> Option<f64> {
    let start = id3v2_end(file)?;
    let mut buf = vec![0u8; MP3_SEARCH];
    let n = read_at(file, start, &mut buf)?;
    let buf = &buf[..n];
    // The first frame: a header whose next frame also starts with a matching header, so a
    // stray 0xFF in leftover tag bytes isn't taken for one.
    let (offset, frame) = (0..n.saturating_sub(4)).find_map(|i| {
        let f = frame_header(&buf[i..])?;
        let next = buf.get(i + f.length..)?;
        let g = frame_header(next)?;
        (g.sample_rate == f.sample_rate && g.layer == f.layer).then_some((i, f))
    })?;
    let frame_bytes = &buf[offset..];

    if frame.layer == 3 {
        let side_info = match (frame.mpeg1, frame.mono) {
            (true, false) => 32,
            (true, true) => 17,
            (false, false) => 17,
            (false, true) => 9,
        };
        let xing = frame_bytes.get(4 + side_info..4 + side_info + 12);
        if let Some(x) = xing.filter(|x| &x[..4] == b"Xing" || &x[..4] == b"Info") {
            if be32(&x[4..8]) & 1 != 0 {
                let frames = be32(&x[8..12]);
                if frames > 0 {
                    return Some(f64::from(frames) * f64::from(frame.samples) / f64::from(frame.sample_rate));
                }
            }
        }
        if let Some(v) = frame_bytes.get(36..54).filter(|v| &v[..4] == b"VBRI") {
            let frames = be32(&v[14..18]);
            if frames > 0 {
                return Some(f64::from(frames) * f64::from(frame.samples) / f64::from(frame.sample_rate));
            }
        }
    }

    // Constant bitrate: the audio bytes at the first frame's bitrate.
    let mut tail = [0u8; 3];
    let id3v1 = if len >= 128 && read_at(file, len - 128, &mut tail) == Some(3) && &tail == b"TAG" { 128 } else { 0 };
    let audio = len.checked_sub(start + offset as u64 + id3v1)?;
    Some(audio as f64 * 8.0 / f64::from(frame.bitrate))
}

// ---- MP4 / M4A ------------------------------------------------------------------------------

/// The (offset, size, header length) of each box between `start` and `end`.
fn mp4_boxes(file: &mut File, start: u64, end: u64) -> impl Iterator<Item = ([u8; 4], u64, u64, u64)> + '_ {
    let mut pos = start;
    std::iter::from_fn(move || {
        if pos + 8 > end {
            return None;
        }
        let mut h = [0u8; 16];
        let n = read_at(file, pos, &mut h)?;
        if n < 8 {
            return None;
        }
        let kind = [h[4], h[5], h[6], h[7]];
        let (size, header) = match be32(&h[..4]) {
            0 => (end - pos, 8),
            1 if n >= 16 => (be64(&h[8..16]), 16),
            1 => return None,
            s => (u64::from(s), 8),
        };
        if size < header || pos + size > end {
            return None;
        }
        let at = pos;
        pos += size;
        Some((kind, at, size, header))
    })
}

fn mp4(file: &mut File, len: u64) -> Option<f64> {
    let (_, moov, size, header) = mp4_boxes(file, 0, len).find(|b| &b.0 == b"moov")?;
    let (_, mvhd, _, mvhd_header) = mp4_boxes(file, moov + header, moov + size).find(|b| &b.0 == b"mvhd")?;
    let mut b = [0u8; 32];
    let n = read_at(file, mvhd + mvhd_header, &mut b)?;
    let (timescale, duration) = match b[0] {
        0 if n >= 20 => (be32(&b[12..16]), u64::from(be32(&b[16..20]))),
        1 if n >= 32 => (be32(&b[20..24]), be64(&b[24..32])),
        _ => return None,
    };
    (timescale > 0).then(|| duration as f64 / f64::from(timescale))
}

// ---- FLAC -----------------------------------------------------------------------------------

fn flac(file: &mut File) -> Option<f64> {
    // "fLaC", then STREAMINFO is always the first metadata block.
    let mut b = [0u8; 4 + 4 + 18];
    if read_at(file, 0, &mut b)? < b.len() || b[4] & 0x7F != 0 {
        return None;
    }
    let s = &b[8..];
    let rate = (u32::from(s[10]) << 12) | (u32::from(s[11]) << 4) | (u32::from(s[12]) >> 4);
    let total = (u64::from(s[13] & 0x0F) << 32) | u64::from(be32(&s[14..18]));
    (rate > 0 && total > 0).then(|| total as f64 / f64::from(rate))
}

// ---- WAV ------------------------------------------------------------------------------------

fn wav(file: &mut File, len: u64) -> Option<f64> {
    let mut pos = 12;
    let mut byte_rate = None;
    while pos + 8 <= len {
        let mut h = [0u8; 20];
        let n = read_at(file, pos, &mut h)?;
        if n < 8 {
            return None;
        }
        let size = u64::from(le32(&h[4..8]));
        match &h[..4] {
            b"fmt " if n >= 20 => byte_rate = Some(le32(&h[16..20])),
            b"data" => {
                let rate = byte_rate.filter(|r| *r > 0)?;
                // A streamed WAV may leave the size unset; the rest of the file is the data.
                let size = if size == 0 || size == u64::from(u32::MAX) { len - pos - 8 } else { size.min(len - pos - 8) };
                return Some(size as f64 / f64::from(rate));
            }
            _ => {}
        }
        pos += 8 + size + (size & 1);
    }
    None
}

// ---- Ogg ------------------------------------------------------------------------------------

fn ogg(file: &mut File, len: u64) -> Option<f64> {
    // The first page carries the codec's identification header.
    let mut first = [0u8; 27 + 255 + 19];
    let n = read_at(file, 0, &mut first)?;
    let segments = *first.get(26)? as usize;
    let packet = first.get(27 + segments..n)?;
    let (rate, pre_skip) = if packet.starts_with(b"\x01vorbis") && packet.len() >= 16 {
        (le32(&packet[12..16]), 0)
    } else if packet.starts_with(b"OpusHead") && packet.len() >= 12 {
        (48_000, u64::from(u16::from_le_bytes([packet[10], packet[11]])))
    } else {
        return None;
    };

    // The last page's granule position is the stream's length in samples.
    let from = len.saturating_sub(OGG_TAIL);
    let mut tail = vec![0u8; (len - from) as usize];
    let n = read_at(file, from, &mut tail)?;
    let tail = &tail[..n];
    let page = tail.windows(4).rposition(|w| w == b"OggS")?;
    let granule = tail.get(page + 6..page + 14)?;
    let granule = u64::from_le_bytes(granule.try_into().ok()?);
    (rate > 0 && granule > pre_skip && granule != u64::MAX).then(|| (granule - pre_skip) as f64 / f64::from(rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A file holding `bytes`, removed when dropped.
    struct TempFile(PathBuf);

    impl TempFile {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn file(bytes: &[u8]) -> TempFile {
        static N: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!("pinepods-duration-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&path, bytes).unwrap();
        TempFile(path)
    }

    /// MPEG1 layer III, 128 kbps, 44.1 kHz, stereo, no padding: 417-byte frames of 1152 samples.
    const HEADER: [u8; 4] = [0xFF, 0xFB, 0x90, 0x00];

    fn frames(n: usize) -> Vec<u8> {
        let mut out = Vec::new();
        for _ in 0..n {
            out.extend_from_slice(&HEADER);
            out.extend(std::iter::repeat_n(0u8, 413));
        }
        out
    }

    fn id3(size: usize) -> Vec<u8> {
        let mut t = b"ID3\x04\x00\x00".to_vec();
        t.extend([(size >> 21) as u8 & 0x7F, (size >> 14) as u8 & 0x7F, (size >> 7) as u8 & 0x7F, size as u8 & 0x7F]);
        t.extend(std::iter::repeat_n(0u8, size));
        t
    }

    #[test]
    fn constant_bitrate_mp3_from_its_size() {
        // 1000 frames of 1152 samples at 44.1 kHz = 26.12s.
        let mut bytes = id3(5000);
        bytes.extend(frames(1000));
        bytes.extend(b"TAG");
        bytes.extend(std::iter::repeat_n(0u8, 125));
        assert_eq!(audio_duration(file(&bytes).path()), Some(26));
    }

    #[test]
    fn vbr_mp3_from_its_xing_header() {
        let mut first = frames(1);
        first[4 + 32..4 + 32 + 4].copy_from_slice(b"Xing");
        first[4 + 32 + 4..4 + 32 + 8].copy_from_slice(&1u32.to_be_bytes());
        first[4 + 32 + 8..4 + 32 + 12].copy_from_slice(&100_000u32.to_be_bytes());
        let mut bytes = id3(300);
        bytes.extend(first);
        bytes.extend(frames(10));
        // 100,000 frames * 1152 / 44100 = 2612.2s, whatever the file's size.
        assert_eq!(audio_duration(file(&bytes).path()), Some(2612));
    }

    #[test]
    fn mp4_from_its_movie_header() {
        let mut mvhd = vec![0u8; 8 + 100];
        mvhd[..4].copy_from_slice(&108u32.to_be_bytes());
        mvhd[4..8].copy_from_slice(b"mvhd");
        mvhd[8 + 12..8 + 16].copy_from_slice(&1000u32.to_be_bytes());
        mvhd[8 + 16..8 + 20].copy_from_slice(&3_725_400u32.to_be_bytes());
        let mut moov = ((8 + mvhd.len()) as u32).to_be_bytes().to_vec();
        moov.extend(b"moov");
        moov.extend(&mvhd);
        let mut bytes = b"\x00\x00\x00\x10ftypM4A \x00\x00\x00\x00".to_vec();
        bytes.extend(b"\x00\x00\x00\x10mdat\x00\x00\x00\x00\x00\x00\x00\x00");
        bytes.extend(moov);
        assert_eq!(audio_duration(file(&bytes).path()), Some(3725));
    }

    #[test]
    fn flac_from_its_streaminfo() {
        let mut bytes = b"fLaC\x80\x00\x00\x22".to_vec();
        let mut info = [0u8; 34];
        // 44100 Hz (20 bits), stereo, 16 bit, 4,410,000 samples = 100s.
        let rate = 44100u32;
        info[10] = (rate >> 12) as u8;
        info[11] = (rate >> 4) as u8;
        info[12] = ((rate & 0x0F) << 4) as u8 | (1 << 1);
        info[13] = 0xF0;
        info[14..18].copy_from_slice(&4_410_000u32.to_be_bytes());
        bytes.extend(info);
        assert_eq!(audio_duration(file(&bytes).path()), Some(100));
    }

    #[test]
    fn wav_from_its_data_chunk() {
        let byte_rate = 44100u32 * 4;
        let data = byte_rate as usize * 3;
        let mut bytes = b"RIFF\x00\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x02\x00".to_vec();
        bytes.extend(44100u32.to_le_bytes());
        bytes.extend(byte_rate.to_le_bytes());
        bytes.extend(b"\x04\x00\x10\x00data");
        bytes.extend((data as u32).to_le_bytes());
        bytes.extend(std::iter::repeat_n(0u8, data));
        assert_eq!(audio_duration(file(&bytes).path()), Some(3));
    }

    fn ogg_page(granule: u64, packet: &[u8]) -> Vec<u8> {
        let mut p = b"OggS\x00\x02".to_vec();
        p.extend(granule.to_le_bytes());
        p.extend([0u8; 12]);
        p.push(1);
        p.push(packet.len() as u8);
        p.extend(packet);
        p
    }

    #[test]
    fn opus_from_its_last_granule() {
        let mut head = b"OpusHead\x01\x02".to_vec();
        head.extend(312u16.to_le_bytes());
        head.extend([0u8; 7]);
        let mut bytes = ogg_page(0, &head);
        bytes.extend(ogg_page(0, b"OpusTags"));
        bytes.extend(ogg_page(48_000 * 90 + 312, &[0u8; 50]));
        assert_eq!(audio_duration(file(&bytes).path()), Some(90));
    }

    #[test]
    fn vorbis_from_its_last_granule() {
        let mut id = b"\x01vorbis\x00\x00\x00\x00\x02".to_vec();
        id.extend(22050u32.to_le_bytes());
        id.extend([0u8; 14]);
        let mut bytes = ogg_page(0, &id);
        bytes.extend(ogg_page(22050 * 61, &[0u8; 40]));
        assert_eq!(audio_duration(file(&bytes).path()), Some(61));
    }

    #[test]
    fn unrecognised_files_have_no_duration() {
        assert_eq!(audio_duration(file(b"not audio at all, just some text").path()), None);
        assert_eq!(audio_duration(file(&[]).path()), None);
    }
}
