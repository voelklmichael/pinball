//! LHA `-lh5-` decompression (static Huffman blocks, 8 KiB dictionary).
//!
//! This is the algorithm from Haruhiko Okumura's `ar002`, which the game uses
//! for `CMP1` files (decoder at 0x4571fc / 0x4572a0 in `Timeshock!.exe`).

use std::fmt;

const DICBIT: u32 = 13;
const MAXMATCH: usize = 256;
const THRESHOLD: usize = 3;
/// Literals 0..=255 plus match lengths 3..=256.
const NC: usize = 255 + MAXMATCH + 2 - THRESHOLD;
const CBIT: u32 = 9;
const NP: usize = DICBIT as usize + 1;
const NT: usize = 16 + 3;
const PBIT: u32 = 4;
const TBIT: u32 = 5;
const NPT: usize = if NT > NP { NT } else { NP };

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    BadTable,
    BackReferenceBeforeStart { pos: usize, dist: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadTable => write!(f, "invalid Huffman table"),
            Error::BackReferenceBeforeStart { pos, dist } => {
                write!(f, "back-reference before start of output (pos {pos}, dist {dist})")
            }
        }
    }
}

impl std::error::Error for Error {}

/// MSB-first bit reader. Reads past the end return zero bits, as in `ar002`.
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u64,
    count: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0, buf: 0, count: 0 }
    }

    fn refill(&mut self) {
        while self.count <= 56 {
            let byte = self.data.get(self.pos).copied().unwrap_or(0);
            self.pos += 1;
            self.buf |= u64::from(byte) << (56 - self.count);
            self.count += 8;
        }
    }

    /// The next 16 bits, without consuming them.
    fn peek16(&mut self) -> u32 {
        self.refill();
        (self.buf >> 48) as u32
    }

    fn skip(&mut self, n: u32) {
        self.refill();
        self.buf <<= n;
        self.count -= n;
    }

    fn bits(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        self.refill();
        let v = (self.buf >> (64 - n)) as u32;
        self.skip(n);
        v
    }
}

/// Canonical Huffman lookup: direct table for short codes, a binary tree
/// (`left`/`right`, node ids >= `nchar`) for codes longer than `table_bits`.
struct Huffman {
    table: Vec<u16>,
    left: Vec<u16>,
    right: Vec<u16>,
}

enum Slot {
    Table(usize),
    Left(usize),
    Right(usize),
}

impl Huffman {
    fn new(table_bits: u32, nchar: usize) -> Self {
        Self {
            table: vec![0; 1 << table_bits],
            left: vec![0; 2 * nchar - 1],
            right: vec![0; 2 * nchar - 1],
        }
    }

    fn fill(&mut self, symbol: u16) {
        self.table.fill(symbol);
    }

    fn slot(&mut self, slot: &Slot) -> &mut u16 {
        match *slot {
            Slot::Table(i) => &mut self.table[i],
            Slot::Left(i) => &mut self.left[i],
            Slot::Right(i) => &mut self.right[i],
        }
    }

    /// `make_table` from ar002.
    fn build(&mut self, bit_len: &[u8], table_bits: u32) -> Result<(), Error> {
        let nchar = bit_len.len();
        let mut count = [0u32; 17];
        for &len in bit_len {
            count[usize::from(len)] += 1;
        }
        let mut start = [0u32; 18];
        for i in 1..=16 {
            start[i + 1] = start[i] + (count[i] << (16 - i));
        }
        if start[17] != 1 << 16 {
            return Err(Error::BadTable);
        }

        let jut_bits = 16 - table_bits;
        let mut weight = [0u32; 17];
        for i in 1..=table_bits as usize {
            start[i] >>= jut_bits;
            weight[i] = 1 << (table_bits as usize - i);
        }
        for i in table_bits as usize + 1..=16 {
            weight[i] = 1 << (16 - i);
        }

        let table_len = 1usize << table_bits;
        let first_unused = (start[table_bits as usize + 1] >> jut_bits) as usize;
        if first_unused < table_len {
            self.table[first_unused..].fill(0);
        }

        let mut avail = nchar;
        let mask = 1u32 << (15 - table_bits);
        for (ch, &len) in bit_len.iter().enumerate() {
            let len = usize::from(len);
            if len == 0 {
                continue;
            }
            let next_code = start[len] + weight[len];
            if len <= table_bits as usize {
                let (lo, hi) = (start[len] as usize, next_code as usize);
                if hi > table_len {
                    return Err(Error::BadTable);
                }
                self.table[lo..hi].fill(ch as u16);
            } else {
                let mut k = start[len];
                let mut slot = Slot::Table((k >> jut_bits) as usize);
                for _ in 0..len - table_bits as usize {
                    if *self.slot(&slot) == 0 {
                        self.left[avail] = 0;
                        self.right[avail] = 0;
                        *self.slot(&slot) = avail as u16;
                        avail += 1;
                    }
                    let node = usize::from(*self.slot(&slot));
                    slot = if k & mask != 0 { Slot::Right(node) } else { Slot::Left(node) };
                    k <<= 1;
                }
                *self.slot(&slot) = ch as u16;
            }
            start[len] = next_code;
        }
        Ok(())
    }

    /// Decode one symbol, using `table_bits` of `peek` for the direct lookup.
    fn decode(&self, peek: u32, table_bits: u32, nchar: usize) -> usize {
        let mut j = usize::from(self.table[(peek >> (16 - table_bits)) as usize]);
        let mut mask = 1u32 << (15 - table_bits);
        while j >= nchar {
            j = usize::from(if peek & mask != 0 { self.right[j] } else { self.left[j] });
            mask >>= 1;
        }
        j
    }
}

struct Decoder<'a> {
    bits: BitReader<'a>,
    block_size: u32,
    c_len: [u8; NC],
    c: Huffman,
    pt_len: [u8; NPT],
    pt: Huffman,
}

impl<'a> Decoder<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            bits: BitReader::new(data),
            block_size: 0,
            c_len: [0; NC],
            c: Huffman::new(12, NC),
            pt_len: [0; NPT],
            pt: Huffman::new(8, NPT),
        }
    }

    fn read_pt_len(&mut self, nn: usize, nbit: u32, special: Option<usize>) -> Result<(), Error> {
        let n = self.bits.bits(nbit) as usize;
        if n == 0 {
            let c = self.bits.bits(nbit) as u16;
            self.pt_len[..nn].fill(0);
            self.pt.fill(c);
            return Ok(());
        }
        let mut i = 0;
        while i < n.min(nn) {
            let peek = self.bits.peek16();
            let mut c = peek >> 13;
            if c == 7 {
                let mut mask = 1u32 << 12;
                while peek & mask != 0 {
                    mask >>= 1;
                    c += 1;
                }
            }
            self.bits.skip(if c < 7 { 3 } else { c - 3 });
            self.pt_len[i] = c as u8;
            i += 1;
            if Some(i) == special {
                let zeros = self.bits.bits(2) as usize;
                let end = (i + zeros).min(nn);
                self.pt_len[i..end].fill(0);
                i = end;
            }
        }
        self.pt_len[i..nn].fill(0);
        self.pt.build(&self.pt_len[..nn], 8)
    }

    fn read_c_len(&mut self) -> Result<(), Error> {
        let n = self.bits.bits(CBIT) as usize;
        if n == 0 {
            let c = self.bits.bits(CBIT) as u16;
            self.c_len.fill(0);
            self.c.fill(c);
            return Ok(());
        }
        let mut i = 0;
        while i < n.min(NC) {
            let peek = self.bits.peek16();
            let c = self.pt.decode(peek, 8, NT);
            self.bits.skip(u32::from(self.pt_len[c]));
            if c <= 2 {
                let zeros = match c {
                    0 => 1,
                    1 => self.bits.bits(4) as usize + 3,
                    _ => self.bits.bits(CBIT) as usize + 20,
                };
                let end = (i + zeros).min(NC);
                self.c_len[i..end].fill(0);
                i = end;
            } else {
                self.c_len[i] = (c - 2) as u8;
                i += 1;
            }
        }
        self.c_len[i..].fill(0);
        self.c.build(&self.c_len, 12)
    }

    fn decode_c(&mut self) -> Result<usize, Error> {
        if self.block_size == 0 {
            self.block_size = self.bits.bits(16);
            self.read_pt_len(NT, TBIT, Some(3))?;
            self.read_c_len()?;
            self.read_pt_len(NP, PBIT, None)?;
        }
        self.block_size = self.block_size.wrapping_sub(1);
        let peek = self.bits.peek16();
        let j = self.c.decode(peek, 12, NC);
        self.bits.skip(u32::from(self.c_len[j]));
        Ok(j)
    }

    fn decode_p(&mut self) -> usize {
        let peek = self.bits.peek16();
        let j = self.pt.decode(peek, 8, NP);
        self.bits.skip(u32::from(self.pt_len[j]));
        if j == 0 { 0 } else { (1 << (j - 1)) + self.bits.bits(j as u32 - 1) as usize }
    }
}

/// Decompress an lh5 bitstream into exactly `out_len` bytes.
pub fn decompress(data: &[u8], out_len: usize) -> Result<Vec<u8>, Error> {
    let mut d = Decoder::new(data);
    let mut out = Vec::with_capacity(out_len);
    while out.len() < out_len {
        let c = d.decode_c()?;
        if c <= usize::from(u8::MAX) {
            out.push(c as u8);
            continue;
        }
        let len = c - (usize::from(u8::MAX) + 1 - THRESHOLD);
        let dist = d.decode_p();
        let pos = out.len();
        let src = pos
            .checked_sub(dist + 1)
            .ok_or(Error::BackReferenceBeforeStart { pos, dist })?;
        for k in 0..len.min(out_len - pos) {
            out.push(out[src + k]);
        }
    }
    Ok(out)
}
