use std::collections::{BTreeSet, HashMap, HashSet};

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::models::SpotifyTrack;

pub const MATCH_THRESHOLD: f64 = 0.75;

const TITLE_WEIGHT: f64 = 0.55;
const ARTIST_WEIGHT: f64 = 0.30;
const DURATION_WEIGHT: f64 = 0.15;
const MIN_TITLE_SIMILARITY: f64 = 0.8;
const ARTIST_NAME_SIMILARITY: f64 = 0.85;
const QUALIFIER_MISMATCH_FLOOR: f64 = 0.4;
const RAREST_TOKENS_PER_QUERY: usize = 3;
const NEUTRAL: f64 = 0.5;

const DESCRIPTOR_WORDS: &[&str] = &[
    "original", "mix", "extended", "version", "edit", "radio", "single", "album", "clean", "explicit", "remastered", "remaster",
    "digital", "mono", "stereo",
];
const VERSION_KEYWORDS: &[&str] = &[
    "remix", "rmx", "mix", "edit", "dub", "bootleg", "vip", "rework", "flip", "mashup", "version", "acoustic", "live", "instrumental",
    "cover", "demo", "session", "reprise", "acapella", "refix", "remake", "part", "pt",
];
const IGNORED_QUALIFIER_WORDS: &[&str] = &["extended", "radio", "original"];
const FEAT_WORDS: &[&str] = &["feat", "ft", "featuring"];
const ARTIST_SEPARATORS: &[&str] = &[
    " feat. ", " feat ", " ft. ", " ft ", " featuring ", " and ", " x ", " vs. ", " vs ", " with ", " + ", ",", ";", "/",
];

#[derive(Debug, Clone)]
pub struct LocalTrack {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackMatch {
    pub local_track_id: i64,
    pub score: f64,
}

struct ParsedTitle {
    base: String,
    qualifiers: BTreeSet<String>,
}

struct Entry {
    id: i64,
    title: ParsedTitle,
    artists: Vec<String>,
    duration_secs: f64,
}

pub struct LibraryIndex {
    entries: Vec<Entry>,
    by_base: HashMap<String, Vec<usize>>,
    by_token: HashMap<String, Vec<usize>>,
}

fn fold(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.nfd().filter(|c| !is_combining_mark(*c)).flat_map(char::to_lowercase) {
        match c {
            'ø' => out.push('o'),
            'ß' => out.push_str("ss"),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            'đ' => out.push('d'),
            'ł' => out.push('l'),
            '’' | '\'' | '`' => {}
            '&' => out.push_str(" and "),
            '–' | '—' => out.push('-'),
            c => out.push(c),
        }
    }
    out
}

fn words(folded: &str) -> Vec<&str> {
    folded.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect()
}

fn normalise_words(folded: &str) -> String {
    words(folded).join(" ")
}

fn is_year(word: &str) -> bool {
    word.len() == 4 && (word.starts_with("19") || word.starts_with("20")) && word.chars().all(|c| c.is_ascii_digit())
}

fn classify_qualifier(group: &str, force_keep: bool) -> Option<String> {
    let group_words = words(group);
    let first = *group_words.first()?;
    if FEAT_WORDS.contains(&first) {
        return None;
    }
    if group_words.iter().all(|w| DESCRIPTOR_WORDS.contains(w) || is_year(w)) {
        return None;
    }
    let has_keyword = group_words.iter().any(|w| VERSION_KEYWORDS.contains(w));
    if !has_keyword && !force_keep {
        return None;
    }
    let kept: Vec<&str> = group_words.into_iter().filter(|w| !IGNORED_QUALIFIER_WORDS.contains(w)).collect();
    (!kept.is_empty()).then(|| kept.join(" "))
}

fn split_groups(folded: &str) -> (String, Vec<String>) {
    let mut rest = String::new();
    let mut groups = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for c in folded.chars() {
        match c {
            '(' | '[' => {
                if depth == 0 {
                    rest.push(' ');
                } else {
                    current.push(' ');
                }
                depth += 1;
            }
            ')' | ']' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    groups.push(std::mem::take(&mut current));
                }
            }
            c if depth > 0 => current.push(c),
            c => rest.push(c),
        }
    }
    if depth > 0 {
        groups.push(current);
    }
    (rest, groups)
}

fn strip_feat_clause(text: &str) -> &str {
    let mut offset = 0;
    for piece in text.split_inclusive(|c: char| !c.is_alphanumeric()) {
        let word = piece.trim_end_matches(|c: char| !c.is_alphanumeric());
        if FEAT_WORDS.contains(&word) {
            return &text[..offset];
        }
        offset += piece.len();
    }
    text
}

fn parse_title(raw: &str) -> ParsedTitle {
    let folded = fold(raw);
    let (rest, groups) = split_groups(&folded);
    let (base_part, suffix) = match rest.split_once(" - ") {
        Some((base, suffix)) => (base.to_string(), Some(suffix.to_string())),
        None => (rest, None),
    };
    let mut qualifiers = BTreeSet::new();
    qualifiers.extend(groups.iter().filter_map(|g| classify_qualifier(g, false)));
    if let Some(suffix) = suffix {
        qualifiers.extend(classify_qualifier(&suffix, true));
    }
    let mut base = normalise_words(strip_feat_clause(&base_part));
    if base.is_empty() {
        base = normalise_words(&folded);
    }
    ParsedTitle { base, qualifiers }
}

fn split_artists<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for name in names {
        let mut padded = format!(" {} ", fold(name));
        for separator in ARTIST_SEPARATORS {
            padded = padded.replace(separator, "|");
        }
        for part in padded.split('|') {
            let normalised = normalise_words(part);
            let normalised = normalised.strip_prefix("the ").unwrap_or(&normalised).to_string();
            if !normalised.is_empty() && seen.insert(normalised.clone()) {
                out.push(normalised);
            }
        }
    }
    out
}

fn levenshtein_ratio(a: &[char], b: &[char]) -> f64 {
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    1.0 - previous[b.len()] as f64 / longest as f64
}

fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let spaced = levenshtein_ratio(&a.chars().collect::<Vec<_>>(), &b.chars().collect::<Vec<_>>());
    let compact = levenshtein_ratio(
        &a.chars().filter(|c| *c != ' ').collect::<Vec<_>>(),
        &b.chars().filter(|c| *c != ' ').collect::<Vec<_>>(),
    );
    spaced.max(compact)
}

fn title_score(query: &ParsedTitle, local: &ParsedTitle) -> f64 {
    let base = similarity(&query.base, &local.base);
    if base < MIN_TITLE_SIMILARITY {
        return 0.0;
    }
    let factor = if query.qualifiers.is_empty() && local.qualifiers.is_empty() {
        1.0
    } else {
        let shared = query.qualifiers.intersection(&local.qualifiers).count() as f64;
        let total = query.qualifiers.union(&local.qualifiers).count() as f64;
        QUALIFIER_MISMATCH_FLOOR + (1.0 - QUALIFIER_MISMATCH_FLOOR) * shared / total
    };
    base * factor
}

fn artist_score(query: &[String], local: &[String]) -> f64 {
    if query.is_empty() || local.is_empty() {
        return NEUTRAL;
    }
    let (smaller, larger) = if query.len() <= local.len() { (query, local) } else { (local, query) };
    let overlapping = smaller
        .iter()
        .filter(|s| larger.iter().any(|l| similarity(s, l) >= ARTIST_NAME_SIMILARITY))
        .count();
    overlapping as f64 / smaller.len() as f64
}

fn duration_score(query_secs: f64, local_secs: f64) -> f64 {
    if query_secs <= 0.0 || local_secs <= 0.0 || !local_secs.is_finite() {
        return NEUTRAL;
    }
    match (query_secs - local_secs).abs() {
        d if d <= 3.0 => 1.0,
        d if d <= 8.0 => 0.85,
        d if d <= 20.0 => 0.6,
        d if d <= 60.0 => 0.4,
        _ => 0.25,
    }
}

impl LibraryIndex {
    pub fn new(tracks: &[LocalTrack]) -> Self {
        let mut entries = Vec::with_capacity(tracks.len());
        let mut by_base: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_token: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, track) in tracks.iter().enumerate() {
            let title = parse_title(&track.title);
            by_base.entry(title.base.clone()).or_default().push(index);
            for token in title.base.split(' ').collect::<HashSet<_>>() {
                by_token.entry(token.to_string()).or_default().push(index);
            }
            entries.push(Entry {
                id: track.id,
                title,
                artists: split_artists([track.artist.as_str()]),
                duration_secs: track.duration_secs,
            });
        }
        Self { entries, by_base, by_token }
    }

    fn candidates(&self, base: &str) -> Vec<usize> {
        let mut out: Vec<usize> = self.by_base.get(base).cloned().unwrap_or_default();
        let mut postings: Vec<&Vec<usize>> = base.split(' ').filter_map(|t| self.by_token.get(t)).collect();
        postings.sort_by_key(|p| p.len());
        for posting in postings.into_iter().take(RAREST_TOKENS_PER_QUERY) {
            out.extend_from_slice(posting);
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn best_match(&self, title: &str, artists: &[String], duration_ms: u64) -> Option<TrackMatch> {
        let query_title = parse_title(title);
        let query_artists = split_artists(artists.iter().map(String::as_str));
        let query_secs = duration_ms as f64 / 1000.0;
        let mut best: Option<TrackMatch> = None;
        for index in self.candidates(&query_title.base) {
            let entry = &self.entries[index];
            let title_part = title_score(&query_title, &entry.title);
            if title_part == 0.0 {
                continue;
            }
            let score = TITLE_WEIGHT * title_part
                + ARTIST_WEIGHT * artist_score(&query_artists, &entry.artists)
                + DURATION_WEIGHT * duration_score(query_secs, entry.duration_secs);
            let better = best.is_none_or(|b| score > b.score + 1e-9 || ((score - b.score).abs() <= 1e-9 && entry.id < b.local_track_id));
            if better {
                best = Some(TrackMatch { local_track_id: entry.id, score });
            }
        }
        best.filter(|m| m.score >= MATCH_THRESHOLD)
    }

    pub fn annotate(&self, tracks: &mut [SpotifyTrack]) {
        for track in tracks {
            let found = self.best_match(&track.name, &track.artists, track.duration_ms);
            track.local_track_id = found.map(|m| m.local_track_id);
            track.match_score = found.map(|m| (m.score * 1000.0).round() / 1000.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(id: i64, title: &str, artist: &str, secs: f64) -> LocalTrack {
        LocalTrack { id, title: title.into(), artist: artist.into(), duration_secs: secs }
    }

    fn find(index: &LibraryIndex, title: &str, artists: &[&str], secs: u64) -> Option<i64> {
        let artists: Vec<String> = artists.iter().map(|a| a.to_string()).collect();
        index.best_match(title, &artists, secs * 1000).map(|m| m.local_track_id)
    }

    #[test]
    fn exact_match() {
        let index = LibraryIndex::new(&[local(1, "Strobe", "deadmau5", 637.0), local(2, "Ghosts n Stuff", "deadmau5", 330.0)]);
        let m = index.best_match("Strobe", &["deadmau5".to_string()], 637_000).unwrap();
        assert_eq!(m.local_track_id, 1);
        assert!(m.score > 0.99);
    }

    #[test]
    fn feat_variants_are_equivalent() {
        let index = LibraryIndex::new(&[local(1, "Wake Me Up", "Avicii", 247.0)]);
        assert_eq!(find(&index, "Wake Me Up (feat. Aloe Blacc)", &["Avicii", "Aloe Blacc"], 247), Some(1));
        assert_eq!(find(&index, "Wake Me Up - feat. Aloe Blacc", &["Avicii"], 247), Some(1));
        let index = LibraryIndex::new(&[local(1, "Wake Me Up ft. Aloe Blacc", "Avicii feat. Aloe Blacc", 247.0)]);
        assert_eq!(find(&index, "Wake Me Up", &["Avicii", "Aloe Blacc"], 247), Some(1));
    }

    #[test]
    fn original_and_extended_mix_equivalence() {
        let index = LibraryIndex::new(&[local(1, "Ghosts 'n' Stuff (Original Mix)", "deadmau5", 330.0)]);
        assert_eq!(find(&index, "Ghosts 'n' Stuff - Original Mix", &["deadmau5"], 330), Some(1));
        assert_eq!(find(&index, "Ghosts 'n' Stuff", &["deadmau5"], 330), Some(1));
        let index = LibraryIndex::new(&[local(1, "Opus", "Eric Prydz", 540.0)]);
        assert_eq!(find(&index, "Opus - Extended Mix", &["Eric Prydz"], 540), Some(1));
        assert_eq!(find(&index, "Opus (Radio Edit)", &["Eric Prydz"], 200), Some(1));
    }

    #[test]
    fn remix_does_not_match_original_in_either_direction() {
        let originals = LibraryIndex::new(&[local(1, "Sunrise", "Norah Jones", 200.0)]);
        assert_eq!(find(&originals, "Sunrise (Kaskade Remix)", &["Norah Jones"], 200), None);
        assert_eq!(find(&originals, "Sunrise - Kaskade Remix", &["Norah Jones"], 200), None);
        let remixes = LibraryIndex::new(&[local(1, "Sunrise (Kaskade Remix)", "Norah Jones", 200.0)]);
        assert_eq!(find(&remixes, "Sunrise", &["Norah Jones"], 200), None);
        assert_eq!(find(&remixes, "Sunrise - Kaskade Remix", &["Norah Jones"], 200), Some(1));
        assert_eq!(find(&remixes, "Sunrise (Other Remix)", &["Norah Jones"], 200), None);
    }

    #[test]
    fn picks_the_right_remix_among_several() {
        let index = LibraryIndex::new(&[
            local(1, "Sunrise", "Norah Jones", 200.0),
            local(2, "Sunrise (Kaskade Remix)", "Norah Jones", 200.0),
            local(3, "Sunrise (Other Remix)", "Norah Jones", 200.0),
        ]);
        assert_eq!(find(&index, "Sunrise - Kaskade Remix", &["Norah Jones"], 200), Some(2));
        assert_eq!(find(&index, "Sunrise", &["Norah Jones"], 200), Some(1));
    }

    #[test]
    fn diacritics_and_punctuation_are_ignored() {
        let index = LibraryIndex::new(&[local(1, "Halo", "Beyoncé", 261.0), local(2, "Kickstart My Heart", "Mötley Crüe", 283.0)]);
        assert_eq!(find(&index, "Halo", &["Beyonce"], 261), Some(1));
        assert_eq!(find(&index, "Kickstart My Heart", &["Motley Crue"], 283), Some(2));
        let index = LibraryIndex::new(&[local(1, "Don't Stop Me Now!", "Queen", 209.0)]);
        assert_eq!(find(&index, "Dont Stop Me Now", &["Queen"], 209), Some(1));
    }

    #[test]
    fn duration_breaks_ties_between_versions() {
        let index = LibraryIndex::new(&[local(1, "Opus", "Eric Prydz", 540.0), local(2, "Opus (Radio Edit)", "Eric Prydz", 210.0)]);
        assert_eq!(find(&index, "Opus", &["Eric Prydz"], 212), Some(2));
        assert_eq!(find(&index, "Opus - Extended Mix", &["Eric Prydz"], 538), Some(1));
    }

    #[test]
    fn very_different_duration_is_penalised_not_rejected() {
        let index = LibraryIndex::new(&[local(1, "Opus", "Eric Prydz", 210.0)]);
        let m = index.best_match("Opus", &["Eric Prydz".to_string()], 540_000).unwrap();
        assert!(m.score >= MATCH_THRESHOLD && m.score < 0.95);
    }

    #[test]
    fn same_title_by_a_different_artist_does_not_match() {
        let index = LibraryIndex::new(&[local(1, "Intro", "Some Band", 100.0)]);
        assert_eq!(find(&index, "Intro", &["The xx"], 100), None);
    }

    #[test]
    fn multiple_artist_forms_overlap() {
        let index = LibraryIndex::new(&[local(1, "Cola", "Camelphat & Elderbrook", 215.0)]);
        assert_eq!(find(&index, "Cola", &["CamelPhat", "Elderbrook"], 215), Some(1));
        let index = LibraryIndex::new(&[local(1, "Cola", "The Prodigy", 215.0)]);
        assert_eq!(find(&index, "Cola", &["Prodigy"], 215), Some(1));
    }

    #[test]
    fn annotate_sets_fields_and_leaves_unmatched_empty() {
        let index = LibraryIndex::new(&[local(7, "Strobe", "deadmau5", 637.0)]);
        let make = |name: &str| SpotifyTrack {
            id: name.into(),
            name: name.into(),
            artists: vec!["deadmau5".into()],
            album: String::new(),
            duration_ms: 637_000,
            isrc: None,
            image_url: None,
            local_track_id: None,
            match_score: None,
        };
        let mut tracks = vec![make("Strobe"), make("Unknown Track")];
        index.annotate(&mut tracks);
        assert_eq!(tracks[0].local_track_id, Some(7));
        assert!(tracks[0].match_score.unwrap() > 0.99);
        assert_eq!(tracks[1].local_track_id, None);
        assert_eq!(tracks[1].match_score, None);
    }

    #[test]
    fn handles_a_large_library_through_the_index() {
        let tracks: Vec<LocalTrack> = (0..20_000).map(|i| local(i, &format!("Track Number {i} Alpha"), &format!("Artist {}", i % 500), 200.0)).collect();
        let index = LibraryIndex::new(&tracks);
        assert_eq!(find(&index, "Track Number 12345 Alpha", &["Artist 345"], 200), Some(12345));
    }
}
