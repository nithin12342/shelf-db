// fsst.rs - symbol table training and string encoding (MF-05).
//
// A string block repeats substrings. This codec learns a table of up to 255
// substrings of two to four bytes and stores each string as table codes. A byte
// with no table symbol is written as the escape code 255 followed by the byte.
//
// Training is gain-driven and greedy:
//   1. Count every 2, 3, and 4 byte substring that no earlier symbol covers.
//   2. Score each candidate as count * (len - 1) - (len + 1). The first term
//      is the bytes saved, the second is the table entry cost.
//   3. Keep the best candidate. Stop at 255 symbols or when the best score is
//      not positive.
// After each pick, every byte that the symbol covers is removed from the
// counts, so a later pick never counts a saved byte twice.
//
// Layout: count u32 | table | per string: code count u32 | codes.

use std::collections::HashMap;

use crate::error::{take_u32, CodecError};
use crate::traits::{ColumnCodec, ColumnData, ColumnType, DecodedColumn};

/// The id of the FSST codec.
pub const FSST_ID: u8 = 4;
/// The largest symbol table. Code 255 is the escape, so 255 symbols is the cap.
pub const MAX_SYMBOLS: usize = 255;
/// The escape code. It is never a table index.
pub const ESCAPE: u8 = 255;
/// The shortest symbol.
const MIN_SYMBOL: usize = 2;
/// The longest symbol.
const MAX_SYMBOL: usize = 4;
/// The corpus bytes that training reads. The greedy loop stays bounded.
const TRAIN_BYTES: usize = 16 * 1024;

/// A symbol table for one string block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SymbolTable {
    symbols: Vec<Vec<u8>>,
}

impl SymbolTable {
    /// An empty table. Every byte is then written as an escape pair.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build a table from symbols. The count and the symbol lengths are
    /// checked, so a table from disk can never index out of range.
    pub fn with_symbols(symbols: Vec<Vec<u8>>) -> Result<Self, CodecError> {
        if symbols.len() > MAX_SYMBOLS {
            return Err(CodecError::TooManySymbols(symbols.len()));
        }
        for symbol in &symbols {
            if symbol.len() < MIN_SYMBOL || symbol.len() > MAX_SYMBOL {
                return Err(CodecError::Corrupt("symbol length outside two to four"));
            }
        }
        Ok(Self { symbols })
    }

    /// The number of symbols.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// True when the table holds no symbol.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// The symbols in code order.
    pub fn symbols(&self) -> &[Vec<u8>] {
        &self.symbols
    }

    /// The code for each symbol.
    fn table_map(&self) -> HashMap<&[u8], u8> {
        self.symbols
            .iter()
            .enumerate()
            .map(|(index, symbol)| (symbol.as_slice(), index as u8))
            .collect()
    }

    /// Encode one string. The longest symbol at each position wins.
    pub fn encode(&self, value: &[u8]) -> Vec<u8> {
        let map = self.table_map();
        self.encode_with(&map, value)
    }

    /// Encode one string against a prepared lookup map.
    fn encode_with(&self, map: &HashMap<&[u8], u8>, value: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(value.len());
        let mut at = 0usize;
        while at < value.len() {
            let longest = MAX_SYMBOL.min(value.len() - at);
            let mut matched = false;
            if longest >= MIN_SYMBOL {
                for len in (MIN_SYMBOL..=longest).rev() {
                    if let Some(&code) = map.get(&value[at..at + len]) {
                        out.push(code);
                        at += len;
                        matched = true;
                        break;
                    }
                }
            }
            if !matched {
                out.push(ESCAPE);
                out.push(value[at]);
                at += 1;
            }
        }
        out
    }

    /// Decode one string from table codes.
    pub fn decode(&self, codes: &[u8]) -> Result<Vec<u8>, CodecError> {
        let mut out = Vec::new();
        let mut at = 0usize;
        while at < codes.len() {
            let code = codes[at];
            if code == ESCAPE {
                let byte = *codes
                    .get(at + 1)
                    .ok_or(CodecError::Corrupt("dangling fsst escape"))?;
                out.push(byte);
                at += 2;
            } else {
                let symbol = self
                    .symbols
                    .get(code as usize)
                    .ok_or(CodecError::Corrupt("fsst code outside the table"))?;
                out.extend_from_slice(symbol);
                at += 1;
            }
        }
        Ok(out)
    }

    /// Serialize the table: count u8, then length u8 and bytes per symbol.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + self.symbols.len() * (1 + MAX_SYMBOL));
        out.push(self.symbols.len() as u8);
        for symbol in &self.symbols {
            out.push(symbol.len() as u8);
            out.extend_from_slice(symbol);
        }
        out
    }

    /// Read a table. Returns the table and the bytes it used.
    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), CodecError> {
        let count = *bytes.first().ok_or(CodecError::Corrupt("truncated fsst table"))? as usize;
        let mut at = 1usize;
        let mut symbols = Vec::with_capacity(count);
        for _ in 0..count {
            let len = *bytes.get(at).ok_or(CodecError::Corrupt("truncated fsst symbol"))? as usize;
            at += 1;
            let end = at + len;
            if end > bytes.len() {
                return Err(CodecError::Corrupt("truncated fsst symbol"));
            }
            symbols.push(bytes[at..end].to_vec());
            at = end;
        }
        Ok((Self::with_symbols(symbols)?, at))
    }
}

/// Count the uncovered substrings of length two to four, with their frequency.
fn substring_counts(corpus: &[&[u8]], covered: &[Vec<bool>]) -> HashMap<Vec<u8>, u32> {
    let mut counts: HashMap<Vec<u8>, u32> = HashMap::new();
    for (text, mask) in corpus.iter().zip(covered.iter()) {
        for start in 0..text.len() {
            if mask[start] {
                continue;
            }
            for len in MIN_SYMBOL..=MAX_SYMBOL {
                let end = start + len;
                if end > text.len() {
                    break;
                }
                if mask[end - 1] {
                    continue;
                }
                *counts.entry(text[start..end].to_vec()).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// Mark every occurrence of `symbol` that no earlier symbol covers.
fn mark_covered(corpus: &[&[u8]], covered: &mut [Vec<bool>], symbol: &[u8]) {
    for (text, mask) in corpus.iter().zip(covered.iter_mut()) {
        let mut at = 0usize;
        while at + symbol.len() <= text.len() {
            let end = at + symbol.len();
            let free = !mask[at..end].iter().any(|&slot| slot);
            if free && &text[at..end] == symbol {
                for slot in mask[at..end].iter_mut() {
                    *slot = true;
                }
                at = end;
            } else {
                at += 1;
            }
        }
    }
}

/// Train a symbol table on a block of strings.
pub fn train(values: &[Vec<u8>]) -> SymbolTable {
    let mut corpus: Vec<&[u8]> = Vec::new();
    let mut used = 0usize;
    for value in values.iter() {
        if used >= TRAIN_BYTES {
            break;
        }
        let take = value.len().min(TRAIN_BYTES - used);
        corpus.push(&value[..take]);
        used += take;
    }
    let mut covered: Vec<Vec<bool>> = corpus.iter().map(|text| vec![false; text.len()]).collect();
    let mut table = SymbolTable::empty();

    while table.len() < MAX_SYMBOLS {
        let counts = substring_counts(&corpus, &covered);
        let mut best: Option<(i64, Vec<u8>)> = None;
        for (candidate, count) in counts {
            if count < 2 {
                continue;
            }
            let len = candidate.len() as i64;
            let gain = count as i64 * (len - 1) - (len + 1);
            let better = match &best {
                None => true,
                Some((best_gain, best_symbol)) => {
                    gain > *best_gain
                        || (gain == *best_gain
                            && (candidate.len() > best_symbol.len()
                                || (candidate.len() == best_symbol.len()
                                    && candidate < *best_symbol)))
                }
            };
            if better {
                best = Some((gain, candidate));
            }
        }
        let Some((gain, symbol)) = best else { break };
        if gain <= 0 {
            break;
        }
        mark_covered(&corpus, &mut covered, &symbol);
        table.symbols.push(symbol);
    }
    table
}

/// A codec that stores a string block through a trained symbol table.
#[derive(Clone, Copy, Debug, Default)]
pub struct FsstCodec;

impl ColumnCodec for FsstCodec {
    fn id(&self) -> u8 {
        FSST_ID
    }

    fn name(&self) -> &'static str {
        "fsst"
    }

    fn accepts(&self, ty: ColumnType) -> bool {
        ty == ColumnType::Bytes
    }

    fn encode(&self, data: &ColumnData<'_>) -> Result<Vec<u8>, CodecError> {
        let ColumnData::Bytes(values) = data else {
            return Err(CodecError::UnsupportedType(data.column_type().name()));
        };
        let table = train(values);
        let map = table.table_map();
        let table_bytes = table.to_bytes();
        let mut out = Vec::with_capacity(4 + table_bytes.len() + values.len() * 4);
        out.extend_from_slice(&(values.len() as u32).to_le_bytes());
        out.extend_from_slice(&table_bytes);
        for value in values.iter() {
            let codes = table.encode_with(&map, value);
            out.extend_from_slice(&(codes.len() as u32).to_le_bytes());
            out.extend_from_slice(&codes);
        }
        Ok(out)
    }

    fn decode(&self, bytes: &[u8]) -> Result<DecodedColumn, CodecError> {
        let count = take_u32(bytes, 0)? as usize;
        let (table, used) = SymbolTable::from_bytes(&bytes[4..])?;
        let mut at = 4 + used;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let len = take_u32(bytes, at)? as usize;
            at += 4;
            let end = at
                .checked_add(len)
                .ok_or(CodecError::Corrupt("fsst code length overflow"))?;
            if end > bytes.len() {
                return Err(CodecError::Corrupt("truncated fsst codes"));
            }
            out.push(table.decode(&bytes[at..end])?);
            at = end;
        }
        Ok(DecodedColumn::Bytes(out))
    }

    fn estimate(&self, data: &ColumnData<'_>) -> usize {
        match data {
            ColumnData::Bytes(values) => {
                let table = train(values);
                let mut total = 4 + table.to_bytes().len() + values.len() * 4;
                for value in values.iter() {
                    total += table.encode(value).len();
                }
                total
            }
            _ => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repeated_corpus() -> Vec<Vec<u8>> {
        let phrase = b"the quick brown fox jumps over the lazy dog ";
        (0..200).map(|_| phrase.to_vec()).collect()
    }

    #[test]
    fn training_finds_symbols_and_stays_under_the_cap() {
        let table = train(&repeated_corpus());
        assert!(!table.is_empty(), "a repeated corpus must train symbols");
        assert!(table.len() <= MAX_SYMBOLS);
        for symbol in table.symbols() {
            assert!(symbol.len() >= MIN_SYMBOL && symbol.len() <= MAX_SYMBOL);
        }
    }

    #[test]
    fn a_repeated_corpus_encodes_smaller_than_raw() {
        let codec = FsstCodec;
        let values = repeated_corpus();
        let raw = ColumnData::Bytes(&values).raw_bytes();
        let bytes = codec.encode(&ColumnData::Bytes(&values)).expect("encode");
        assert!(bytes.len() < raw, "encoded {} vs raw {}", bytes.len(), raw);
    }

    #[test]
    fn a_block_round_trips_exactly() {
        let codec = FsstCodec;
        let values = vec![
            b"".to_vec(),
            b"alpha beta gamma".to_vec(),
            b"alpha beta gamma".to_vec(),
            vec![7u8; 4096],
            b"the quick brown fox".to_vec(),
        ];
        let bytes = codec.encode(&ColumnData::Bytes(&values)).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");
        assert_eq!(decoded.as_bytes().expect("bytes").to_vec(), values);
    }

    #[test]
    fn distinct_short_values_round_trip_with_an_empty_table() {
        let codec = FsstCodec;
        let values = vec![b"a".to_vec(), b"b".to_vec(), b"".to_vec()];
        let bytes = codec.encode(&ColumnData::Bytes(&values)).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");
        assert_eq!(decoded.as_bytes().expect("bytes").to_vec(), values);
    }

    #[test]
    fn a_code_outside_the_table_is_rejected() {
        let table = SymbolTable::with_symbols(vec![b"ab".to_vec()]).expect("table");
        assert_eq!(
            table.decode(&[5]),
            Err(CodecError::Corrupt("fsst code outside the table"))
        );
        assert_eq!(
            table.decode(&[ESCAPE]),
            Err(CodecError::Corrupt("dangling fsst escape"))
        );
    }

    #[test]
    fn a_table_round_trips_through_bytes() {
        let table = train(&repeated_corpus());
        let bytes = table.to_bytes();
        let (restored, used) = SymbolTable::from_bytes(&bytes).expect("table from bytes");
        assert_eq!(used, bytes.len());
        assert_eq!(restored, table);
    }

    #[test]
    fn too_many_symbols_are_rejected() {
        let symbols = vec![b"ab".to_vec(); MAX_SYMBOLS + 1];
        assert_eq!(
            SymbolTable::with_symbols(symbols),
            Err(CodecError::TooManySymbols(MAX_SYMBOLS + 1))
        );
    }

    #[test]
    fn a_symbol_of_one_byte_is_rejected() {
        assert!(matches!(
            SymbolTable::with_symbols(vec![b"a".to_vec()]),
            Err(CodecError::Corrupt(_))
        ));
    }

    #[test]
    fn codec_rejects_a_numeric_block() {
        let codec = FsstCodec;
        let values = vec![1u64];
        assert_eq!(
            codec.encode(&ColumnData::U64(&values)),
            Err(CodecError::UnsupportedType("u64"))
        );
    }
}
