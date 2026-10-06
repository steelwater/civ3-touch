//! GOG compatibility boundary. No Android types or game rules live here.
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    panic::catch_unwind,
    path::{Path, PathBuf},
};

pub const PROFILE: &str = "gog-1471405734-english-2.0.0.7-m2";
struct Entry {
    path: &'static str,
    kind: &'static str,
    required: bool,
    limit: usize,
}
const ENTRIES: &[Entry] = &[
    Entry {
        path: "goggame-1471405734.info",
        kind: "identity",
        required: true,
        limit: 16384,
    },
    Entry {
        path: "Text/version.txt",
        kind: "version",
        required: true,
        limit: 1024,
    },
    Entry {
        path: "Conquests/readme.txt",
        kind: "conquests",
        required: true,
        limit: 131072,
    },
    Entry {
        path: "Art/Units/Settler/settler.ini",
        kind: "unit",
        required: true,
        limit: 16384,
    },
    Entry {
        path: "Art/Terrain/xggc.pcx",
        kind: "terrain",
        required: true,
        limit: 2097152,
    },
    Entry {
        path: "Art/Terrain/xtgc.pcx",
        kind: "terrain",
        required: true,
        limit: 2097152,
    },
    Entry {
        path: "Art/Terrain/xdgc.pcx",
        kind: "terrain",
        required: true,
        limit: 2097152,
    },
    Entry {
        path: "Art/Terrain/xpgc.pcx",
        kind: "terrain",
        required: true,
        limit: 2097152,
    },
    Entry {
        path: "Art/Units/Settler/settDefault.flc",
        kind: "idle",
        required: true,
        limit: 1048576,
    },
    Entry {
        path: "Art/Units/Settler/settRun.flc",
        kind: "run",
        required: true,
        limit: 1048576,
    },
    Entry {
        path: "Art/Units/Settler/SetRunFoot1.wav",
        kind: "step",
        required: false,
        limit: 1048576,
    },
    Entry {
        path: "Sounds/Build/ancient/AncECfull.mp3",
        kind: "music",
        required: false,
        limit: 8388608,
    },
];

pub fn profile() -> Value {
    json!({"id": PROFILE, "files": ENTRIES.iter().map(|e| json!({"path":e.path,"kind":e.kind,"required":e.required,"limit":e.limit})).collect::<Vec<_>>()})
}

/// Resolve one profile path case-insensitively, rejecting ambiguity and symlinks.
/// Only allowlisted relative names cross this boundary, never arbitrary INI paths.
fn resolve(root: &Path, relative: &str) -> Result<Option<PathBuf>, String> {
    let mut path = root.to_path_buf();
    for component in relative.split('/') {
        let mut matches = fs::read_dir(&path).map_err(|_| format!("Cannot read folder for {relative}. Copy the installation to readable local storage."))?
            .filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(component));
        let Some(found) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Err(format!(
                "Ambiguous filename casing: {relative}. Remove duplicate copies from the source."
            ));
        }
        if found.file_type().map_err(|e| e.to_string())?.is_symlink() {
            return Err(format!(
                "Symbolic links are not supported: {relative}. Copy the actual files."
            ));
        }
        path = found.path();
    }
    Ok(Some(path))
}

fn read_entry(root: &Path, entry: &Entry) -> Result<Option<Vec<u8>>, String> {
    let Some(path) = resolve(root, entry.path)? else {
        return if entry.required {
            Err(format!(
                "Missing {}. Select the installation root containing Art, Text and Conquests.",
                entry.path
            ))
        } else {
            Ok(None)
        };
    };
    if !path.is_file() {
        return Err(format!("Not a regular readable file: {}", entry.path));
    }
    let file = fs::File::open(path)
        .map_err(|_| format!("Unreadable {}. Copy the file again.", entry.path))?;
    let mut bytes = Vec::new();
    file.take(entry.limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| format!("Read failed: {}", entry.path))?;
    if bytes.is_empty() || bytes.len() > entry.limit {
        return Err(format!(
            "Unsupported file size: {}. Use the supported English GOG installation.",
            entry.path
        ));
    }
    Ok(Some(bytes))
}

fn u16_at(b: &[u8], n: usize) -> usize {
    u16::from_le_bytes([b[n], b[n + 1]]) as usize
}
fn u32_at(b: &[u8], n: usize) -> usize {
    u32::from_le_bytes([b[n], b[n + 1], b[n + 2], b[n + 3]]) as usize
}

pub struct Image {
    pub key: String,
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub frames: usize,
    pub delay: usize,
}
pub struct Assets {
    pub images: Vec<Image>,
    pub audio: Value,
    pub warnings: Vec<String>,
}
impl Assets {
    pub fn metadata(&self) -> Value {
        json!({"profile":PROFILE,"images":self.images.iter().map(|i| json!({"key":i.key,"width":i.width,"height":i.height,"frames":i.frames,"delay":i.delay})).collect::<Vec<_>>(),"audio":self.audio,"warnings":self.warnings})
    }
}

fn verify_marker(kind: &str, bytes: &[u8]) -> Result<(), String> {
    let text = String::from_utf8_lossy(bytes);
    let supported = match kind {
        "identity" => {
            let value: Value =
                serde_json::from_str(&text).map_err(|_| "Invalid GOG metadata JSON")?;
            value["gameId"] == "1471405734"
                && value["rootGameId"] == "1471405734"
                && value["language"] == "english"
        }
        "version" => text.replace('\r', "").trim() == "#VERSION\n1.29f",
        "conquests" => {
            text.contains("SID MEIER'S CIVILIZATION III: Conquests README")
                && text.lines().any(|l| l.trim() == "v1.22")
        }
        "unit" => {
            let mut section = String::new();
            let mut idle = false;
            let mut run = false;
            for line in text.lines().map(str::trim) {
                if line.starts_with('[') {
                    section = line.to_ascii_lowercase();
                }
                if section == "[animations]" {
                    if let Some((key, value)) = line.split_once('=') {
                        if key.trim().eq_ignore_ascii_case("DEFAULT") {
                            idle = value.trim().eq_ignore_ascii_case("settDefault.flc");
                        }
                        if key.trim().eq_ignore_ascii_case("RUN") {
                            run = value.trim().eq_ignore_ascii_case("settRun.flc");
                        }
                    }
                }
            }
            idle && run
        }
        _ => false,
    };
    if supported {
        Ok(())
    } else {
        Err("Unsupported installation. Use the English GOG Civilization III Complete 2.0.0.7 layout; Steam/CD/modded layouts are not supported.".into())
    }
}

/// Bounded validation before calling the unchanged upstream PCX decoder, which
/// otherwise accepts truncated scanlines and can overflow invalid coordinates.
fn terrain(bytes: &[u8]) -> Result<fc3_pcx::PcxImage, String> {
    if bytes.len() < 897
        || bytes[0..4] != [10, 5, 1, 8]
        || bytes[65] != 1
        || u16_at(bytes, 4) != 0
        || u16_at(bytes, 6) != 0
        || u16_at(bytes, 8) != 1151
        || u16_at(bytes, 10) != 575
        || u16_at(bytes, 66) != 1152
        || bytes[bytes.len() - 769] != 12
    {
        return Err("Unsupported terrain PCX header (expected 1152×576 indexed image)".into());
    }
    let end = bytes.len() - 769;
    let mut offset = 128;
    for _ in 0..576 {
        let mut columns = 0;
        while columns < 1152 {
            if offset >= end {
                return Err("Truncated PCX pixels".into());
            }
            let value = bytes[offset];
            offset += 1;
            let count = if value & 0xc0 == 0xc0 {
                if offset >= end || value & 63 == 0 {
                    return Err("Invalid PCX run".into());
                }
                offset += 1;
                (value & 63) as usize
            } else {
                1
            };
            columns += count;
            if columns > 1152 {
                return Err("PCX run crosses a scanline".into());
            }
        }
    }
    if offset != end {
        return Err("Unexpected PCX trailing pixels".into());
    }
    fc3_pcx::read_pcx(bytes).map_err(|e| e.to_string())
}

fn animation(bytes: &[u8], kind: &str) -> Result<Image, String> {
    let (w, h, frames) = if kind == "idle" {
        (30, 55, 15)
    } else {
        (40, 63, 10)
    };
    if bytes.len() < 128
        || u32_at(bytes, 0) != bytes.len()
        || u16_at(bytes, 4) != 0xaf12
        || u16_at(bytes, 8) != w
        || u16_at(bytes, 10) != h
        || u16_at(bytes, 12) != 8
        || u16_at(bytes, 96) != 8
        || u16_at(bytes, 98) != frames
    {
        return Err("Unsupported Settler animation header or truncated file".into());
    }
    // Bound frame/chunk walks; also reject chunk types the pinned decoder cannot handle.
    let mut at = 128;
    for _ in 0..8 * (frames + 1) {
        if at + 16 > bytes.len() {
            return Err("Truncated FLC frame".into());
        }
        let end = at
            .checked_add(u32_at(bytes, at))
            .ok_or("Invalid frame size")?;
        if end > bytes.len() || end <= at + 16 || u16_at(bytes, at + 4) != 0xf1fa {
            return Err("Invalid FLC frame".into());
        }
        let chunks = u16_at(bytes, at + 6);
        let mut chunk = at + 16;
        for _ in 0..chunks {
            if chunk + 6 > end {
                return Err("Truncated FLC chunk".into());
            }
            let raw_size = u32_at(bytes, chunk);
            let size = if raw_size == 0xcdcdcdcd && u16_at(bytes, chunk + 4) == 4 {
                778
            } else {
                raw_size
            };
            if size < 6
                || chunk.checked_add(size).is_none_or(|n| n > end)
                || ![4, 7, 15].contains(&u16_at(bytes, chunk + 4))
            {
                return Err("Unsupported FLC chunk".into());
            }
            chunk += size;
        }
        at = end;
    }
    let decoded = catch_unwind(|| fc3_flic::read_flic(bytes))
        .map_err(|_| "Malformed animation rejected by decoder")?
        .map_err(|e| e.to_string())?;
    // Eight direction columns, frame rows. Ring frames are excluded, and the
    // resulting texture stays below conservative Android texture-size limits.
    let stride = w * h * 4;
    let mut rgba = vec![0; stride * frames * 8];
    for direction in 0..8 {
        for frame in 0..frames {
            for row in 0..h {
                let src = (direction * (frames + 1) + frame) * stride + row * w * 4;
                let dst = ((frame * h + row) * w * 8 + direction * w) * 4;
                rgba[dst..dst + w * 4].copy_from_slice(&decoded.frame_data[src..src + w * 4]);
            }
        }
    }
    Ok(Image {
        key: kind.into(),
        width: w * 8,
        height: h * frames,
        rgba,
        frames,
        delay: decoded.frame_delay.clamp(40, 200) as usize,
    })
}

/// Validate the complete profile and decode required media before publication.
pub fn load(root: &Path) -> Result<Assets, String> {
    let mut assets = Assets {
        images: Vec::new(),
        audio: json!({}),
        warnings: Vec::new(),
    };
    for entry in ENTRIES {
        let result = (|| -> Result<(), String> {
            let Some(bytes) = read_entry(root, entry)? else {
                assets.warnings.push(format!(
                    "Optional {} is missing; that audio is unavailable.",
                    entry.kind
                ));
                return Ok(());
            };
            match entry.kind {
                "identity" | "version" | "conquests" | "unit" => verify_marker(entry.kind, &bytes)?,
                "terrain" => {
                    let decoded = terrain(&bytes)?;
                    let selections: &[(&str, usize)] = match entry.path {
                        "Art/Terrain/xggc.pcx" => &[("Grassland", 0), ("Coast", 80)],
                        "Art/Terrain/xtgc.pcx" => &[("Tundra", 0)],
                        "Art/Terrain/xdgc.pcx" => &[("Desert", 0)],
                        _ => &[("Plains", 0)],
                    };
                    for &(key, index) in selections {
                        let mut rgba = Vec::with_capacity(128 * 64 * 4);
                        for row in 0..64 {
                            let start = ((index / 9 * 64 + row) * 1152 + index % 9 * 128) * 4;
                            rgba.extend_from_slice(&decoded.data[start..start + 128 * 4]);
                        }
                        assets.images.push(Image {
                            key: key.into(),
                            width: 128,
                            height: 64,
                            rgba,
                            frames: 1,
                            delay: 0,
                        });
                    }
                }
                "idle" | "run" => assets.images.push(animation(&bytes, entry.kind)?),
                "step" => {
                    if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
                        return Err("Unsupported WAV header".into());
                    }
                    assets.audio[entry.kind] = json!(entry.path);
                }
                "music" => {
                    if bytes.len() < 3
                        || !(bytes.starts_with(b"ID3")
                            || (bytes[0] == 255 && bytes[1] & 0xe0 == 0xe0))
                    {
                        return Err("Unsupported MP3 header".into());
                    }
                    assets.audio[entry.kind] = json!(entry.path);
                }
                _ => unreachable!(),
            }
            Ok(())
        })();
        if let Err(error) = result {
            let error = format!("{}: {error}", entry.path);
            if entry.required {
                return Err(error);
            } else {
                assets.warnings.push(error);
            }
        }
    }
    Ok(assets)
}

#[cfg(test)]
mod tests;
