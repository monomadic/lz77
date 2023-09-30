//! LZ77 is a lossless sliding window data compression algorithm. It replaces repeated occurrences of data with references to a single copy.

mod decompress;

pub use decompress::decompress;
