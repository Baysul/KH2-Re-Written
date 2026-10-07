//! Explicit International-system text profile from pinned OpenKH decoder models.
//! Copyright 2019-2022 OpenKH Team and Contributors; Apache-2.0, see NOTICE.
//! Source bytes remain authoritative; Unicode is not a reversible glyph identity.
use crate::message::system_message_tokens;
use sha2::{Digest, Sha256};
use std::io;

#[derive(Clone, Copy)]
enum CharacterMapping {
    Text(char),
    Complex(&'static str),
    Unsupported(u8),
    Tabulation,
}
use CharacterMapping::{Complex, Tabulation, Text, Unsupported};

// Public model table, not text extracted from an installed game.
const INTERNATIONAL: [CharacterMapping; 224] = [
    Text('\u{2b1b}'),
    Text('\u{ff10}'),
    Text('\u{ff11}'),
    Text('\u{ff12}'),
    Text('\u{ff13}'),
    Text('\u{ff14}'),
    Text('\u{ff15}'),
    Text('\u{ff16}'),
    Text('\u{ff17}'),
    Text('\u{ff18}'),
    Text('\u{ff19}'),
    Text('\u{2b}'),
    Text('\u{2212}'),
    Text('\u{2093}'),
    Text('\u{41}'),
    Text('\u{42}'),
    Text('\u{43}'),
    Text('\u{44}'),
    Text('\u{45}'),
    Text('\u{46}'),
    Text('\u{47}'),
    Text('\u{48}'),
    Text('\u{49}'),
    Text('\u{4a}'),
    Text('\u{4b}'),
    Text('\u{4c}'),
    Text('\u{4d}'),
    Text('\u{4e}'),
    Text('\u{4f}'),
    Text('\u{50}'),
    Text('\u{51}'),
    Text('\u{52}'),
    Text('\u{53}'),
    Text('\u{54}'),
    Text('\u{55}'),
    Text('\u{56}'),
    Text('\u{57}'),
    Text('\u{58}'),
    Text('\u{59}'),
    Text('\u{5a}'),
    Text('\u{21}'),
    Text('\u{3f}'),
    Text('\u{25}'),
    Text('\u{2f}'),
    Text('\u{203b}'),
    Text('\u{3001}'),
    Text('\u{3002}'),
    Text('\u{2e}'),
    Text('\u{2c}'),
    Text('\u{3b}'),
    Text('\u{3a}'),
    Text('\u{2026}'),
    Complex("\u{2d}"),
    Text('\u{2013}'),
    Text('\u{301c}'),
    Complex("\u{27}"),
    Unsupported(0x58),
    Unsupported(0x59),
    Text('\u{28}'),
    Text('\u{29}'),
    Text('\u{300c}'),
    Text('\u{300d}'),
    Text('\u{300e}'),
    Text('\u{300f}'),
    Text('\u{201c}'),
    Text('\u{201d}'),
    Text('\u{5b}'),
    Text('\u{5d}'),
    Text('\u{3c}'),
    Text('\u{3e}'),
    Text('\u{2d}'),
    Complex("\u{2013}"),
    Text('\u{2937}'),
    Text('\u{2669}'),
    Text('\u{21fe}'),
    Text('\u{21fd}'),
    Text('\u{25ef}'),
    Text('\u{2715}'),
    Unsupported(0x6e),
    Unsupported(0x6e),
    Unsupported(0x70),
    Unsupported(0x71),
    Unsupported(0x72),
    Tabulation,
    Complex("\u{49}"),
    Complex("\u{49}\u{49}"),
    Complex("\u{49}\u{49}\u{49}"),
    Complex("\u{49}\u{56}"),
    Complex("\u{56}"),
    Complex("\u{56}\u{49}"),
    Complex("\u{56}\u{49}\u{49}"),
    Complex("\u{56}\u{49}\u{49}\u{49}"),
    Complex("\u{49}\u{58}"),
    Complex("\u{58}"),
    Complex("\u{58}\u{49}\u{49}\u{49}"),
    Text('\u{3b1}'),
    Text('\u{3b2}'),
    Text('\u{3b3}'),
    Text('\u{2b51}'),
    Text('\u{2b52}'),
    Complex("\u{58}\u{49}"),
    Complex("\u{58}\u{49}\u{49}"),
    Text('\u{26}'),
    Text('\u{23}'),
    Text('\u{ae}'),
    Text('\u{25b4}'),
    Text('\u{25be}'),
    Text('\u{25b8}'),
    Text('\u{25c2}'),
    Text('\u{b0}'),
    Complex("\u{266a}"),
    Unsupported(0x8f),
    Text('\u{30}'),
    Text('\u{31}'),
    Text('\u{32}'),
    Text('\u{33}'),
    Text('\u{34}'),
    Text('\u{35}'),
    Text('\u{36}'),
    Text('\u{37}'),
    Text('\u{38}'),
    Text('\u{39}'),
    Text('\u{61}'),
    Text('\u{62}'),
    Text('\u{63}'),
    Text('\u{64}'),
    Text('\u{65}'),
    Text('\u{66}'),
    Text('\u{67}'),
    Text('\u{68}'),
    Text('\u{69}'),
    Text('\u{6a}'),
    Text('\u{6b}'),
    Text('\u{6c}'),
    Text('\u{6d}'),
    Text('\u{6e}'),
    Text('\u{6f}'),
    Text('\u{70}'),
    Text('\u{71}'),
    Text('\u{72}'),
    Text('\u{73}'),
    Text('\u{74}'),
    Text('\u{75}'),
    Text('\u{76}'),
    Text('\u{77}'),
    Text('\u{78}'),
    Text('\u{79}'),
    Text('\u{7a}'),
    Text('\u{c6}'),
    Text('\u{e6}'),
    Text('\u{df}'),
    Text('\u{e0}'),
    Text('\u{e1}'),
    Text('\u{e2}'),
    Text('\u{e4}'),
    Text('\u{e8}'),
    Text('\u{e9}'),
    Text('\u{ea}'),
    Text('\u{eb}'),
    Text('\u{ec}'),
    Text('\u{ed}'),
    Text('\u{ee}'),
    Text('\u{ef}'),
    Text('\u{f1}'),
    Text('\u{f2}'),
    Text('\u{f3}'),
    Text('\u{f4}'),
    Text('\u{f6}'),
    Text('\u{f9}'),
    Text('\u{fa}'),
    Text('\u{fb}'),
    Text('\u{fc}'),
    Text('\u{ba}'),
    Text('\u{2014}'),
    Text('\u{bb}'),
    Text('\u{ab}'),
    Text('\u{c0}'),
    Text('\u{c1}'),
    Text('\u{c2}'),
    Text('\u{c4}'),
    Text('\u{c8}'),
    Text('\u{c9}'),
    Text('\u{ca}'),
    Text('\u{cb}'),
    Text('\u{cc}'),
    Text('\u{cd}'),
    Text('\u{ce}'),
    Text('\u{cf}'),
    Text('\u{d1}'),
    Text('\u{d2}'),
    Text('\u{d3}'),
    Text('\u{d4}'),
    Text('\u{d6}'),
    Text('\u{d9}'),
    Text('\u{da}'),
    Text('\u{db}'),
    Text('\u{dc}'),
    Text('\u{a1}'),
    Text('\u{bf}'),
    Text('\u{c7}'),
    Text('\u{e7}'),
    Text('\u{201b}'),
    Text('\u{2019}'),
    Text('\u{60}'),
    Text('\u{b4}'),
    Text('\u{22}'),
    Text('\u{27}'),
    Text('\u{2605}'),
    Text('\u{2606}'),
    Text('\u{25a0}'),
    Text('\u{25a1}'),
    Text('\u{25b2}'),
    Text('\u{25b3}'),
    Text('\u{25cf}'),
    Text('\u{25cb}'),
    Text('\u{266a}'),
    Text('\u{266b}'),
    Text('\u{2192}'),
    Text('\u{2190}'),
    Text('\u{2191}'),
    Text('\u{2193}'),
    Text('\u{30fb}'),
    Text('\u{2764}'),
    Unsupported(0xff),
];

#[derive(Debug, PartialEq, Eq)]
pub enum InternationalContent {
    Text(String),
    Complex(&'static str),
    Command,
    /// Public model value; exact input byte remains in the segment's encoded span.
    Unsupported {
        profile_value: u8,
    },
    UnmappedTable,
}

#[derive(Debug, PartialEq, Eq)]
pub struct InternationalSegment<'a> {
    pub source_offset: usize,
    /// Exact original bytes, including command parameters and terminator.
    pub encoded: &'a [u8],
    pub content: InternationalContent,
}

/// Decode only the explicitly selected pinned International-system profile.
/// Unknown tables are retained instead of substituting the upstream generic '?'.
pub fn decode_international_system(bytes: &[u8]) -> io::Result<Vec<InternationalSegment<'_>>> {
    let tokens = system_message_tokens(bytes)?;
    let mut segments = Vec::new();
    let mut pending = String::new();
    let mut text_start = 0;
    for token in tokens {
        let mapping = match token.opcode {
            1 => Some(Text(' ')),
            2 => Some(Text('\n')),
            0..=0x1f => None,
            _ => Some(INTERNATIONAL[token.opcode as usize - 0x20]),
        };
        if let Some(Text(character)) = mapping {
            if pending.is_empty() {
                text_start = token.source_offset;
            }
            pending.push(character);
            continue;
        }
        if !pending.is_empty() {
            segments.push(InternationalSegment {
                source_offset: text_start,
                encoded: &bytes[text_start..token.source_offset],
                content: InternationalContent::Text(std::mem::take(&mut pending)),
            });
        }
        let content = match mapping {
            Some(Complex(text)) => InternationalContent::Complex(text),
            Some(Unsupported(profile_value)) => InternationalContent::Unsupported { profile_value },
            Some(Tabulation) => InternationalContent::Command,
            None if token.opcode >= 0x19 => InternationalContent::UnmappedTable,
            None => InternationalContent::Command,
            Some(Text(_)) => unreachable!(),
        };
        let end = token.source_offset + 1 + token.parameters.len();
        segments.push(InternationalSegment {
            source_offset: token.source_offset,
            encoded: &bytes[token.source_offset..end],
            content,
        });
    }
    // The strict tokenizer guarantees an End command that flushes pending text.
    Ok(segments)
}

/// Fingerprint ordered content: tag, little-endian byte length, UTF-8 or original data.
/// Commands retain original bytes; unsupported fields fingerprint the public model
/// value, including its alias, while the separate encoded span stays lossless.
pub fn international_fingerprint(segments: &[InternationalSegment<'_>]) -> String {
    let mut hash = Sha256::new();
    for segment in segments {
        let mut profile_byte = [0];
        let (tag, data) = match &segment.content {
            InternationalContent::Text(text) => (1u8, text.as_bytes()),
            InternationalContent::Complex(text) => (2, text.as_bytes()),
            InternationalContent::Command => (3, segment.encoded),
            InternationalContent::Unsupported { profile_value } => {
                profile_byte[0] = *profile_value;
                (4, &profile_byte[..])
            }
            InternationalContent::UnmappedTable => (5, segment.encoded),
        };
        hash.update([tag]);
        hash.update((data.len() as u32).to_le_bytes());
        hash.update(data);
    }
    format!("{:x}", hash.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_generated_profile_matches_independent_pinned_decoder_fingerprint() {
        // Oracle: unchanged OpenKH decoder on authored bytes, never game data.
        let arities = [
            0, 0, 0, 0, 1, 6, 1, 4, 3, 1, 1, 1, 1, 0, 1, 5, 0, 4, 2, 4, 2, 2, 1, 2, 2, 1, 1, 1, 1,
            1, 1, 1,
        ];
        let mut bytes = Vec::new();
        for opcode in 1..=u8::MAX {
            if (0x19..=0x1f).contains(&opcode) {
                continue;
            }
            bytes.push(opcode);
            bytes.extend(vec![0; arities.get(opcode as usize).copied().unwrap_or(0)]);
        }
        bytes.push(0);
        let segments = decode_international_system(&bytes).unwrap();
        assert_eq!(segments.len(), 58);
        let alias = segments
            .iter()
            .find(|segment| segment.encoded[0] == 0x6f)
            .unwrap();
        assert_eq!(alias.encoded, &[0x6f]);
        assert_eq!(
            alias.content,
            InternationalContent::Unsupported {
                profile_value: 0x6e
            }
        );
        let reconstructed: Vec<u8> = segments
            .iter()
            .flat_map(|segment| segment.encoded.iter().copied())
            .collect();
        assert_eq!(reconstructed, bytes);
        assert_eq!(
            international_fingerprint(&segments),
            "b87061c07f59fb249f64b0b3c33887acc122b4980521ac5957a00bc110db16f1"
        );
    }

    #[test]
    fn preserves_complex_symbols_controls_unknowns_and_original_reassembly() {
        let bytes = [0x2e, 0x2f, 0x75, 7, 0, 1, 2, 3, 0x58, 0x19, 0x20, 0x73, 0];
        let segments = decode_international_system(&bytes).unwrap();
        assert_eq!(segments[0].content, InternationalContent::Text("AB".into()));
        assert_eq!(segments[0].encoded, &[0x2e, 0x2f]);
        assert_eq!(segments[1].content, InternationalContent::Complex("II"));
        assert_eq!(segments[1].encoded, &[0x75]);
        assert_eq!(segments[2].content, InternationalContent::Command);
        assert_eq!(segments[2].encoded, &[7, 0, 1, 2, 3]);
        assert_eq!(
            segments[3].content,
            InternationalContent::Unsupported {
                profile_value: 0x58
            }
        );
        assert_eq!(segments[4].content, InternationalContent::UnmappedTable);
        assert_eq!(segments[5].content, InternationalContent::Command);
        let mut reconstructed = Vec::new();
        for segment in &segments {
            assert_eq!(segment.source_offset, reconstructed.len());
            reconstructed.extend_from_slice(segment.encoded);
        }
        assert_eq!(reconstructed, bytes);
    }

    #[test]
    fn preserves_unicode_scalars_whitespace_and_distinct_glyph_encodings() {
        let plain = decode_international_system(&[0x2e, 1, 2, 0x21, 0xfe, 0]).unwrap();
        assert_eq!(
            plain[0].content,
            InternationalContent::Text("A \n０❤".into())
        );
        let roman = decode_international_system(&[0x75, 0]).unwrap();
        let letters = decode_international_system(&[0x36, 0x36, 0]).unwrap();
        assert_eq!(roman[0].content, InternationalContent::Complex("II"));
        assert_eq!(letters[0].content, InternationalContent::Text("II".into()));
        assert_ne!(
            international_fingerprint(&roman),
            international_fingerprint(&letters)
        );
    }

    #[test]
    fn refuses_malformed_records_and_preserves_unmapped_tables_without_question_marks() {
        assert!(decode_international_system(&[0x17, 0]).is_err());
        assert!(decode_international_system(&[0, 0x2e]).is_err());
        let segments = decode_international_system(&[0x19, 0, 0xff, 0]).unwrap();
        assert_eq!(segments[0].content, InternationalContent::UnmappedTable);
        assert_eq!(segments[0].encoded, &[0x19, 0]);
        assert_eq!(
            segments[1].content,
            InternationalContent::Unsupported {
                profile_value: 0xff
            }
        );
    }
}
