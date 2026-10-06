use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "civ3touch-assets-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn put(&self, path: &str, data: impl AsRef<[u8]>) {
        let p = self.0.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }
    fn complete(&self) {
        for e in ENTRIES {
            let b = match e.kind {
                "identity" => {
                    br#"{"gameId":"1471405734","rootGameId":"1471405734","language":"english"}"#
                        .to_vec()
                }
                "version" => b"#VERSION\r\n1.29f\r\n".to_vec(),
                "conquests" => b"SID MEIER'S CIVILIZATION III: Conquests README\nv1.22\n".to_vec(),
                "unit" => b"[Animations]\nDEFAULT=settDefault.flc\nRUN=settRun.flc\n".to_vec(),
                "terrain" => pcx(),
                "idle" | "run" => flc(e.kind),
                _ => continue,
            };
            self.put(e.path, b);
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn put16(b: &mut [u8], at: usize, v: usize) {
    b[at..at + 2].copy_from_slice(&(v as u16).to_le_bytes());
}
fn put32(b: &mut [u8], at: usize, v: usize) {
    b[at..at + 4].copy_from_slice(&(v as u32).to_le_bytes());
}
fn pcx() -> Vec<u8> {
    let mut b = vec![0; 128];
    b[..4].copy_from_slice(&[10, 5, 1, 8]);
    b[65] = 1;
    put16(&mut b, 8, 1151);
    put16(&mut b, 10, 575);
    put16(&mut b, 66, 1152);
    for _ in 0..576 {
        for _ in 0..18 {
            b.extend_from_slice(&[255, 1]);
        }
        b.extend_from_slice(&[210, 1]);
    }
    b.push(12);
    let mut palette = vec![0; 768];
    palette[3..6].copy_from_slice(&[30, 120, 40]);
    b.extend(palette);
    b
}
fn flc(kind: &str) -> Vec<u8> {
    let (w, h, frames) = if kind == "idle" {
        (30, 55, 15)
    } else {
        (40, 63, 10)
    };
    let mut b = vec![0; 128];
    put16(&mut b, 4, 0xaf12);
    put16(&mut b, 8, w);
    put16(&mut b, 10, h);
    put16(&mut b, 12, 8);
    put16(&mut b, 96, 8);
    put16(&mut b, 98, frames);
    for frame_index in 0..8 * (frames + 1) {
        let mut frame = vec![0; 16];
        put16(&mut frame, 4, 0xf1fa);
        put16(&mut frame, 6, 1);
        let mut chunk = vec![0; 6];
        put16(&mut chunk, 4, 15);
        for _ in 0..h {
            let pixel = if frame_index % (frames + 1) == frames {
                254
            } else {
                248 + (frame_index / (frames + 1) % 6) as u8
            };
            chunk.extend_from_slice(&[1, w as u8, pixel]);
        }
        // Upstream consumes a final row packet count but does not dereference it.
        let n = chunk.len();
        put32(&mut chunk, 0, n);
        frame.extend(chunk);
        let n = frame.len();
        put32(&mut frame, 0, n);
        b.extend(frame);
    }
    let n = b.len();
    put32(&mut b, 0, n);
    b
}
#[test]
fn supported_folder_decodes_images_and_reports_optional_audio() {
    let f = Fixture::new();
    f.complete();
    let a = load(&f.0).unwrap();
    assert_eq!(a.images.len(), 7);
    assert_eq!(a.warnings.len(), 2);
    assert_eq!(&a.images[0].rgba[..4], &[30, 120, 40, 255]);
    let idle = a.images.iter().find(|i| i.key == "idle").unwrap();
    assert_eq!(idle.height, 55 * 15);
    assert_eq!(idle.width, 30 * 8);
    assert_eq!(idle.rgba[3], 255);
    assert_eq!(idle.rgba[30 * 4 + 3], 204);
    assert_eq!(idle.rgba[(14 * 55 * 240) * 4 + 3], 255);
    assert_eq!(idle.rgba.len(), 30 * 55 * 15 * 8 * 4);
}
#[test]
fn missing_files_and_other_distributions_are_actionable() {
    let f = Fixture::new();
    assert!(load(&f.0)
        .err()
        .unwrap()
        .contains("goggame-1471405734.info"));
    f.complete();
    f.put(ENTRIES[0].path, br#"{"gameId":"different"}"#);
    assert!(load(&f.0)
        .err()
        .unwrap()
        .contains("Unsupported installation"));
}
#[test]
fn casing_is_resolved_but_duplicate_names_are_rejected() {
    let f = Fixture::new();
    f.complete();
    fs::rename(f.0.join("Art"), f.0.join("ART")).unwrap();
    assert!(load(&f.0).is_ok());
    // Test collision on a case-sensitive directory only (macOS volumes can be insensitive).
    let one = f.0.join("case");
    let two = f.0.join("CASE");
    fs::create_dir(&one).unwrap();
    if fs::create_dir(&two).is_ok() {
        assert!(resolve(&f.0, "case").unwrap_err().contains("Ambiguous"));
    }
}
#[cfg(unix)]
#[test]
fn symbolic_links_are_not_followed() {
    let f = Fixture::new();
    std::os::unix::fs::symlink("/tmp", f.0.join("Art")).unwrap();
    assert!(resolve(&f.0, "Art/Terrain/xggc.pcx")
        .unwrap_err()
        .contains("Symbolic"));
}
#[test]
fn corrupt_or_oversized_images_fail_without_allocating_from_untrusted_dimensions() {
    let mut b = pcx();
    b.drain(128..140);
    assert!(terrain(&b).is_err());
    let mut b = pcx();
    put16(&mut b, 8, 65535);
    assert!(terrain(&b).is_err());
    let mut b = flc("run");
    put16(&mut b, 98, 65535);
    assert!(animation(&b, "run").is_err());
    let mut b = flc("idle");
    put32(&mut b, 128, usize::MAX);
    assert!(animation(&b, "idle").is_err());
    let mut b = flc("idle");
    put16(&mut b, 148, 16);
    assert!(animation(&b, "idle").is_err());
}
#[test]
fn ini_cannot_redirect_reads_outside_the_profile() {
    assert!(verify_marker(
        "unit",
        b"[Animations]\nDEFAULT=../../secret\nRUN=settRun.flc"
    )
    .is_err());
}

#[test]
fn broken_optional_audio_does_not_block_playable_images() {
    let f = Fixture::new();
    f.complete();
    f.put("Art/Units/Settler/SetRunFoot1.wav", b"not a wave file");
    f.put("Sounds/Build/ancient/AncECfull.mp3", b"not an mp3");
    let loaded = load(&f.0).unwrap();
    assert_eq!(loaded.images.len(), 7);
    assert_eq!(loaded.warnings.len(), 2);
    assert!(loaded.audio.as_object().unwrap().is_empty());
}

#[test]
fn empty_files_and_directories_in_place_of_files_are_rejected() {
    let f = Fixture::new();
    f.complete();
    f.put("Art/Units/Settler/settRun.flc", []);
    assert!(load(&f.0).err().unwrap().contains("Unsupported file size"));
    let f = Fixture::new();
    fs::create_dir(f.0.join(ENTRIES[0].path)).unwrap();
    assert!(load(&f.0).err().unwrap().contains("Not a regular"));
}

#[test]
fn bad_animation_packets_become_diagnostics_not_unwinding_callers() {
    let mut b = flc("idle");
    // Literal run claims more bytes than the frame contains.
    b[151] = 127;
    assert!(animation(&b, "idle").is_err());
}
