//! Minimal WebVTT lyrics parser + sync lookup.
//!
//! Only the subset of VTT we care about for music-video subtitles:
//! - header line "WEBVTT" (skipped)
//! - cue blocks: optional ID line, timestamp line, 1+ text lines, blank.
//! - inline HTML/timestamp tags (`<c.colour>`, `<00:00:01.000>`) are stripped.
//!
//! YouTube auto-generated VTTs tend to repeat phrases with progressive word
//! reveal; we de-duplicate consecutive identical lines after stripping tags.

use std::fs;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub start: Duration,
    pub end: Duration,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
}

impl Lyrics {
    pub fn from_vtt_file(path: &Path) -> Option<Self> {
        let raw = fs::read_to_string(path).ok()?;
        Some(Self::from_vtt_str(&raw))
    }

    pub fn from_vtt_str(raw: &str) -> Self {
        let mut lines: Vec<LyricLine> = Vec::new();
        // Normalize line endings.
        let normalized = raw.replace("\r\n", "\n");

        // Split into blocks separated by blank lines.
        for block in normalized.split("\n\n") {
            let block = block.trim_matches(|c: char| c == '\n' || c.is_whitespace());
            if block.is_empty() {
                continue;
            }
            if block.starts_with("WEBVTT") || block.starts_with("NOTE") || block.starts_with("STYLE") {
                continue;
            }

            // Look for a timing line containing " --> ".
            let mut timing_line: Option<&str> = None;
            let mut text_lines: Vec<&str> = Vec::new();
            for l in block.lines() {
                if l.contains("-->") && timing_line.is_none() {
                    timing_line = Some(l);
                } else if timing_line.is_some() {
                    text_lines.push(l);
                }
            }
            let Some(ts) = timing_line else { continue };
            let Some((start, end)) = parse_timing(ts) else { continue };

            let joined = text_lines.join(" ");
            let cleaned = strip_inline_tags(&joined);
            let cleaned = cleaned.trim().to_string();
            if cleaned.is_empty() {
                continue;
            }

            // Skip if same text as previous and overlapping (auto-sub artifact).
            if let Some(prev) = lines.last() {
                if prev.text == cleaned {
                    // Extend the previous cue instead of duplicating it.
                    let last = lines.last_mut().unwrap();
                    if end > last.end {
                        last.end = end;
                    }
                    continue;
                }
            }

            lines.push(LyricLine { start, end, text: cleaned });
        }

        Lyrics { lines }
    }

    /// Cue active at `elapsed`, if any.
    pub fn current_line(&self, elapsed: Duration) -> Option<&LyricLine> {
        // Binary search by start (lines are time-ordered).
        let idx = self
            .lines
            .partition_point(|l| l.start <= elapsed)
            .saturating_sub(1);
        let candidate = self.lines.get(idx)?;
        if elapsed >= candidate.start && elapsed < candidate.end {
            Some(candidate)
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

fn parse_timing(line: &str) -> Option<(Duration, Duration)> {
    let mut parts = line.split("-->");
    let start = parse_timestamp(parts.next()?.trim())?;
    // The end-side may include positioning settings ("00:00:05.000 align:start line:80%").
    let end_raw = parts.next()?.trim();
    let end_ts = end_raw.split_whitespace().next()?;
    let end = parse_timestamp(end_ts)?;
    Some((start, end))
}

fn parse_timestamp(s: &str) -> Option<Duration> {
    // Accepts HH:MM:SS.mmm or MM:SS.mmm
    let segs: Vec<&str> = s.split(':').collect();
    let (h, m, rest) = match segs.as_slice() {
        [h, m, r] => (h.parse::<u64>().ok()?, m.parse::<u64>().ok()?, *r),
        [m, r] => (0u64, m.parse::<u64>().ok()?, *r),
        _ => return None,
    };
    let mut sec_parts = rest.split('.');
    let secs = sec_parts.next()?.parse::<u64>().ok()?;
    let millis = match sec_parts.next() {
        Some(ms) => {
            // Pad/truncate to 3 digits.
            let padded: String = ms.chars().chain(std::iter::repeat('0')).take(3).collect();
            padded.parse::<u64>().ok()?
        }
        None => 0,
    };
    let total = h * 3_600_000 + m * 60_000 + secs * 1000 + millis;
    Some(Duration::from_millis(total))
}

fn strip_inline_tags(s: &str) -> String {
    // Remove anything between '<' and '>', plus the rare "&amp;" → "&".
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for ch in s.chars() {
        match ch {
            '<' => depth += 1,
            '>' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_vtt() {
        let src = "WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nHello world\n\n00:00:04.000 --> 00:00:06.000\nSecond line\n";
        let l = Lyrics::from_vtt_str(src);
        assert_eq!(l.lines.len(), 2);
        assert_eq!(l.lines[0].text, "Hello world");
        assert_eq!(l.lines[0].start, Duration::from_millis(1000));
        assert_eq!(l.lines[0].end, Duration::from_millis(3500));
    }

    #[test]
    fn strips_tags_and_dedups() {
        let src = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n<c.colour>Hi<00:00:01.500>there</c>\n\n00:00:02.000 --> 00:00:04.000\nHithere\n";
        let l = Lyrics::from_vtt_str(src);
        // "Hithere" matches previous text after stripping tags → merged.
        assert_eq!(l.lines.len(), 1);
        assert_eq!(l.lines[0].end, Duration::from_millis(4000));
    }

    #[test]
    fn current_line_lookup() {
        let src = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nA\n\n00:00:05.000 --> 00:00:07.000\nB\n";
        let l = Lyrics::from_vtt_str(src);
        assert!(l.current_line(Duration::from_millis(500)).is_none());
        assert_eq!(l.current_line(Duration::from_millis(2000)).unwrap().text, "A");
        assert!(l.current_line(Duration::from_millis(4000)).is_none());
        assert_eq!(l.current_line(Duration::from_millis(6000)).unwrap().text, "B");
    }
}
