//! A Vorbis I decoder (Xiph's Vorbis I specification) for the music the
//! exe plays through its statically linked libvorbis 1.2.3: codebooks
//! (lookup types 0-2, sparse and ordered lengths), floor 1, residues 0-2,
//! mapping 0 with square-polar coupling, the windowed inverse MDCT and
//! overlap-add. Floor 0 is rejected (no installed stream uses it).
use std::io;

fn invalid(m: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, m.to_owned())
}

/// LSB-first bit reader over a packet.
struct Bits<'a> {
    data: &'a [u8],
    bit: usize,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit: 0 }
    }

    /// `n` bits (n <= 32); None past the end of the packet.
    fn read(&mut self, n: u32) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        if self.bit + n as usize > self.data.len() * 8 {
            self.bit = self.data.len() * 8 + 1;
            return None;
        }
        let mut v: u64 = 0;
        for k in 0..n as usize {
            let b = self.bit + k;
            v |= (((self.data[b >> 3] >> (b & 7)) & 1) as u64) << k;
        }
        self.bit += n as usize;
        Some(v as u32)
    }

    fn flag(&mut self) -> Option<bool> {
        self.read(1).map(|b| b != 0)
    }
}

fn need<T>(v: Option<T>) -> io::Result<T> {
    v.ok_or_else(|| invalid("Vorbis header truncated"))
}

fn ilog(x: u32) -> u32 {
    32 - x.leading_zeros()
}

fn float32_unpack(x: u32) -> f32 {
    let mantissa = (x & 0x1f_ffff) as f64;
    let exponent = ((x & 0x7fe0_0000) >> 21) as i32;
    let v = mantissa * 2f64.powi(exponent - 788);
    (if x & 0x8000_0000 != 0 { -v } else { v }) as f32
}

fn lookup1_values(entries: u32, dimensions: u32) -> u32 {
    let mut r = (entries as f64).powf(1. / dimensions as f64).floor() as u32;
    while (r + 1)
        .checked_pow(dimensions)
        .is_some_and(|p| p <= entries)
    {
        r += 1;
    }
    while r > 0 && r.checked_pow(dimensions).is_none_or(|p| p > entries) {
        r -= 1;
    }
    r
}

/// A codebook: a canonical Huffman tree over its used entries and, for
/// lookup types 1/2, the unpacked VQ vectors.
struct Codebook {
    dimensions: usize,
    /// Tree nodes: [child for bit 0, child for bit 1]; a value with LEAF
    /// set is a leaf holding the entry; 0 is "no child".
    tree: Vec<[u32; 2]>,
    /// VQ vectors, `dimensions` per entry (lookup types 1/2).
    vectors: Vec<f32>,
}

const LEAF: u32 = 0x8000_0000;

impl Codebook {
    fn read(b: &mut Bits) -> io::Result<Self> {
        if need(b.read(24))? != 0x564342 {
            return Err(invalid("Vorbis codebook sync"));
        }
        let dimensions = need(b.read(16))? as usize;
        let entries = need(b.read(24))? as usize;
        if dimensions == 0 || entries == 0 || entries > 1 << 20 {
            return Err(invalid("Vorbis codebook size"));
        }
        let ordered = need(b.flag())?;
        let mut lengths = vec![0u8; entries];
        if !ordered {
            let sparse = need(b.flag())?;
            for l in lengths.iter_mut() {
                if !sparse || need(b.flag())? {
                    *l = need(b.read(5))? as u8 + 1;
                }
            }
        } else {
            let mut length = need(b.read(5))? + 1;
            let mut entry = 0usize;
            while entry < entries {
                let n = need(b.read(ilog((entries - entry) as u32)))? as usize;
                if entry + n > entries || length > 32 {
                    return Err(invalid("Vorbis ordered codebook lengths"));
                }
                lengths[entry..entry + n].fill(length as u8);
                entry += n;
                length += 1;
            }
        }
        let tree = build_tree(&lengths)?;
        let lookup = need(b.read(4))?;
        let vectors = match lookup {
            0 => Vec::new(),
            1 | 2 => {
                let min = float32_unpack(need(b.read(32))?);
                let delta = float32_unpack(need(b.read(32))?);
                let value_bits = need(b.read(4))? + 1;
                let sequence = need(b.flag())?;
                let values = if lookup == 1 {
                    lookup1_values(entries as u32, dimensions as u32) as usize
                } else {
                    entries * dimensions
                };
                let mult = (0..values)
                    .map(|_| need(b.read(value_bits)).map(|v| v as f32))
                    .collect::<io::Result<Vec<_>>>()?;
                let mut out = vec![0f32; entries * dimensions];
                for e in 0..entries {
                    let mut last = 0f32;
                    let mut divisor = 1usize;
                    for i in 0..dimensions {
                        let m = if lookup == 1 {
                            mult[(e / divisor) % values]
                        } else {
                            mult[e * dimensions + i]
                        };
                        let v = m * delta + min + last;
                        out[e * dimensions + i] = v;
                        if sequence {
                            last = v;
                        }
                        if lookup == 1 {
                            divisor *= values;
                        }
                    }
                }
                out
            }
            _ => return Err(invalid("Vorbis codebook lookup type")),
        };
        Ok(Self {
            dimensions,
            tree,
            vectors,
        })
    }

    /// One entry (Huffman decode); None at the end of the packet.
    fn scalar(&self, b: &mut Bits) -> Option<usize> {
        let mut node = 0usize;
        loop {
            let bit = b.read(1)? as usize;
            let next = self.tree.get(node)?[bit];
            if next & LEAF != 0 {
                return Some((next & !LEAF) as usize);
            }
            if next == 0 {
                return None;
            }
            node = next as usize;
        }
    }

    fn vector(&self, b: &mut Bits) -> Option<&[f32]> {
        let e = self.scalar(b)?;
        self.vectors
            .get(e * self.dimensions..(e + 1) * self.dimensions)
    }
}

/// Canonical codewords as the specification assigns them (lowest free
/// code of each length, in entry order), as a binary tree read MSB first.
fn build_tree(lengths: &[u8]) -> io::Result<Vec<[u32; 2]>> {
    let mut tree: Vec<[u32; 2]> = vec![[0, 0]];
    let mut available = [0u32; 33];
    let mut first = true;
    let insert = |tree: &mut Vec<[u32; 2]>, code: u32, len: u8, entry: usize| -> io::Result<()> {
        let mut node = 0usize;
        for k in (0..len).rev() {
            let bit = ((code >> k) & 1) as usize;
            if k == 0 {
                if tree[node][bit] != 0 {
                    return Err(invalid("Vorbis codebook overspecified"));
                }
                tree[node][bit] = LEAF | entry as u32;
            } else {
                let next = tree[node][bit];
                if next & LEAF != 0 {
                    return Err(invalid("Vorbis codebook prefix clash"));
                }
                if next == 0 {
                    tree.push([0, 0]);
                    let id = (tree.len() - 1) as u32;
                    tree[node][bit] = id;
                    node = id as usize;
                } else {
                    node = next as usize;
                }
            }
        }
        Ok(())
    };
    for (entry, &len) in lengths.iter().enumerate() {
        if len == 0 {
            continue;
        }
        let len32 = len as usize;
        if first {
            first = false;
            insert(&mut tree, 0, len, entry)?;
            for (i, a) in available.iter_mut().enumerate().take(len32 + 1).skip(1) {
                *a = 1u32.wrapping_shl(32 - i as u32);
            }
            continue;
        }
        // The deepest free slot no longer than this length.
        let mut z = len32;
        while z > 0 && available[z] == 0 {
            z -= 1;
        }
        if z == 0 {
            return Err(invalid("Vorbis codebook overspecified"));
        }
        let res = available[z];
        available[z] = 0;
        // `res` holds the code left-aligned in 32 bits.
        let code = if len32 == 32 {
            res
        } else {
            res >> (32 - len32)
        };
        insert(&mut tree, code, len, entry)?;
        for y in (z + 1..=len32).rev() {
            available[y] = res + 1u32.wrapping_shl(32 - y as u32);
        }
    }
    Ok(tree)
}

struct Floor1 {
    partition_class: Vec<usize>,
    class_dimensions: Vec<usize>,
    class_subclasses: Vec<u32>,
    class_masterbook: Vec<usize>,
    subclass_books: Vec<Vec<i32>>,
    multiplier: i32,
    x: Vec<i32>,
    /// Indices of `x` in ascending x order.
    sorted: Vec<usize>,
    low: Vec<usize>,
    high: Vec<usize>,
}

impl Floor1 {
    fn read(b: &mut Bits, books: usize) -> io::Result<Self> {
        let partitions = need(b.read(5))? as usize;
        let partition_class: Vec<usize> = (0..partitions)
            .map(|_| need(b.read(4)).map(|v| v as usize))
            .collect::<io::Result<_>>()?;
        let classes = partition_class.iter().max().map_or(0, |m| m + 1);
        let mut class_dimensions = Vec::new();
        let mut class_subclasses = Vec::new();
        let mut class_masterbook = Vec::new();
        let mut subclass_books = Vec::new();
        for _ in 0..classes {
            class_dimensions.push(need(b.read(3))? as usize + 1);
            let sub = need(b.read(2))?;
            class_subclasses.push(sub);
            class_masterbook.push(if sub != 0 {
                need(b.read(8))? as usize
            } else {
                0
            });
            let books_for = (0..1usize << sub)
                .map(|_| need(b.read(8)).map(|v| v as i32 - 1))
                .collect::<io::Result<Vec<_>>>()?;
            if books_for.iter().any(|&k| k >= books as i32) {
                return Err(invalid("Vorbis floor book"));
            }
            subclass_books.push(books_for);
        }
        let multiplier = need(b.read(2))? as i32 + 1;
        let range_bits = need(b.read(4))?;
        let mut x = vec![0, 1 << range_bits];
        for &c in &partition_class {
            for _ in 0..class_dimensions[c] {
                x.push(need(b.read(range_bits))? as i32);
            }
        }
        if x.len() > 65 {
            return Err(invalid("Vorbis floor x list"));
        }
        let mut sorted: Vec<usize> = (0..x.len()).collect();
        sorted.sort_by_key(|&i| x[i]);
        let mut low = vec![0; x.len()];
        let mut high = vec![0; x.len()];
        for i in 2..x.len() {
            let (mut lo, mut hi) = (0usize, 1usize);
            let (mut lx, mut hx) = (-1, i32::MAX);
            for j in 0..i {
                if x[j] < x[i] && x[j] > lx {
                    lx = x[j];
                    lo = j;
                }
                if x[j] > x[i] && x[j] < hx {
                    hx = x[j];
                    hi = j;
                }
            }
            low[i] = lo;
            high[i] = hi;
        }
        Ok(Self {
            partition_class,
            class_dimensions,
            class_subclasses,
            class_masterbook,
            subclass_books,
            multiplier,
            x,
            sorted,
            low,
            high,
        })
    }

    /// Packet decode (Y list), None when unused for this channel.
    fn decode(&self, b: &mut Bits, books: &[Codebook]) -> Option<Vec<i32>> {
        if !b.flag()? {
            return None;
        }
        let range = [256, 128, 86, 64][self.multiplier as usize - 1];
        let bits = ilog(range - 1);
        let mut y = vec![0i32; self.x.len()];
        y[0] = b.read(bits)? as i32;
        y[1] = b.read(bits)? as i32;
        let mut offset = 2;
        for &class in &self.partition_class {
            let cdim = self.class_dimensions[class];
            let cbits = self.class_subclasses[class];
            let csub = (1u32 << cbits) - 1;
            let mut cval = if cbits > 0 {
                books[self.class_masterbook[class]].scalar(b)? as u32
            } else {
                0
            };
            for j in 0..cdim {
                let book = self.subclass_books[class][(cval & csub) as usize];
                cval >>= cbits;
                y[offset + j] = if book >= 0 {
                    books[book as usize].scalar(b)? as i32
                } else {
                    0
                };
            }
            offset += cdim;
        }
        Some(y)
    }

    /// Curve synthesis into `n` floor values (spec 7.2.4).
    fn synthesize(&self, y: &[i32], n: usize, out: &mut [f32]) {
        let range = [256, 128, 86, 64][self.multiplier as usize - 1];
        let count = self.x.len();
        let mut step2 = vec![false; count];
        let mut fy = vec![0i32; count];
        step2[0] = true;
        step2[1] = true;
        fy[0] = y[0];
        fy[1] = y[1];
        for i in 2..count {
            let (lo, hi) = (self.low[i], self.high[i]);
            let predicted = render_point(self.x[lo], fy[lo], self.x[hi], fy[hi], self.x[i]);
            let val = y[i];
            let high_room = range - predicted;
            let low_room = predicted;
            let room = if high_room < low_room {
                high_room * 2
            } else {
                low_room * 2
            };
            if val != 0 {
                step2[lo] = true;
                step2[hi] = true;
                step2[i] = true;
                fy[i] = if val >= room {
                    if high_room > low_room {
                        val - low_room + predicted
                    } else {
                        predicted - val + high_room - 1
                    }
                } else if val & 1 != 0 {
                    predicted - (val + 1) / 2
                } else {
                    predicted + val / 2
                };
            } else {
                step2[i] = false;
                fy[i] = predicted;
            }
        }
        let mut curve = vec![0i32; n];
        let first = self.sorted[0];
        let (mut lx, mut ly) = (0i32, fy[first] * self.multiplier);
        let (mut hx, mut hy) = (0i32, ly);
        for &i in &self.sorted[1..] {
            if step2[i] {
                hy = fy[i] * self.multiplier;
                hx = self.x[i];
                render_line(lx, ly, hx, hy, &mut curve);
                lx = hx;
                ly = hy;
            }
        }
        if (hx as usize) < n {
            render_line(hx, hy, n as i32, hy, &mut curve);
        }
        for (o, &c) in out.iter_mut().zip(&curve) {
            *o = inverse_db(c.clamp(0, 255) as usize);
        }
    }
}

fn render_point(x0: i32, y0: i32, x1: i32, y1: i32, x: i32) -> i32 {
    let dy = y1 - y0;
    let adx = x1 - x0;
    let err = dy.abs() * (x - x0);
    let off = err / adx;
    if dy < 0 { y0 - off } else { y0 + off }
}

fn render_line(x0: i32, y0: i32, x1: i32, y1: i32, v: &mut [i32]) {
    let dy = y1 - y0;
    let adx = x1 - x0;
    if adx <= 0 {
        return;
    }
    let base = dy / adx;
    let sy = if dy < 0 { base - 1 } else { base + 1 };
    let ady = dy.abs() - base.abs() * adx;
    let (mut y, mut err) = (y0, 0);
    if let Some(s) = v.get_mut(x0 as usize) {
        *s = y;
    }
    for x in x0 + 1..x1 {
        err += ady;
        if err >= adx {
            err -= adx;
            y += sy;
        } else {
            y += base;
        }
        if let Some(s) = v.get_mut(x as usize) {
            *s = y;
        }
    }
}

/// floor1_inverse_dB_table: 1.0649863e-07 * 1.0649864^i, i = 0..255.
fn inverse_db(i: usize) -> f32 {
    static TABLE: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let first = 1.0649863e-07f64;
        std::array::from_fn(|i| (first * (1. / first).powf(i as f64 / 255.)) as f32)
    })[i]
}

struct Residue {
    kind: u16,
    begin: usize,
    end: usize,
    partition_size: usize,
    classifications: usize,
    classbook: usize,
    books: Vec<[i32; 8]>,
}

impl Residue {
    fn read(b: &mut Bits, kind: u16, books: usize) -> io::Result<Self> {
        let begin = need(b.read(24))? as usize;
        let end = need(b.read(24))? as usize;
        let partition_size = need(b.read(24))? as usize + 1;
        let classifications = need(b.read(6))? as usize + 1;
        let classbook = need(b.read(8))? as usize;
        let cascade = (0..classifications)
            .map(|_| {
                let low = need(b.read(3))?;
                let high = if need(b.flag())? { need(b.read(5))? } else { 0 };
                Ok(high << 3 | low)
            })
            .collect::<io::Result<Vec<u32>>>()?;
        let books_out = cascade
            .iter()
            .map(|&c| {
                let mut row = [-1i32; 8];
                for (k, r) in row.iter_mut().enumerate() {
                    if c & (1 << k) != 0 {
                        let book = need(b.read(8))? as i32;
                        if book as usize >= books {
                            return Err(invalid("Vorbis residue book"));
                        }
                        *r = book;
                    }
                }
                Ok(row)
            })
            .collect::<io::Result<Vec<_>>>()?;
        if classbook >= books {
            return Err(invalid("Vorbis residue classbook"));
        }
        Ok(Self {
            kind,
            begin,
            end,
            partition_size,
            classifications,
            classbook,
            books: books_out,
        })
    }

    /// Decodes into `vectors` (each n/2 long) for the channels not marked
    /// `skip`.
    fn decode(&self, b: &mut Bits, books: &[Codebook], vectors: &mut [Vec<f32>], skip: &[bool]) {
        if self.kind == 2 {
            if skip.iter().all(|&s| s) {
                return;
            }
            let ch = vectors.len();
            let n = vectors[0].len();
            let mut joint = vec![vec![0f32; n * ch]];
            self.decode_format(b, books, &mut joint, &[false], 1);
            for (i, v) in joint[0].iter().enumerate() {
                vectors[i % ch][i / ch] += v;
            }
        } else {
            self.decode_format(b, books, vectors, skip, self.kind);
        }
    }

    fn decode_format(
        &self,
        b: &mut Bits,
        books: &[Codebook],
        vectors: &mut [Vec<f32>],
        skip: &[bool],
        format: u16,
    ) {
        let size = vectors.first().map_or(0, |v| v.len());
        let begin = self.begin.min(size);
        let end = self.end.min(size);
        let classbook = &books[self.classbook];
        let per_word = classbook.dimensions;
        let to_read = end.saturating_sub(begin);
        let partitions = to_read / self.partition_size;
        if partitions == 0 {
            return;
        }
        let ch = vectors.len();
        let mut classes = vec![vec![0usize; partitions + per_word]; ch];
        for pass in 0..8 {
            let mut count = 0;
            while count < partitions {
                if pass == 0 {
                    for j in 0..ch {
                        if skip[j] {
                            continue;
                        }
                        let Some(mut temp) = classbook.scalar(b) else {
                            return;
                        };
                        for i in (0..per_word).rev() {
                            classes[j][i + count] = temp % self.classifications;
                            temp /= self.classifications;
                        }
                    }
                }
                for _ in 0..per_word {
                    if count >= partitions {
                        break;
                    }
                    for j in 0..ch {
                        if skip[j] {
                            continue;
                        }
                        let book = self.books[classes[j][count]][pass];
                        if book < 0 {
                            continue;
                        }
                        let book = &books[book as usize];
                        if book.vectors.is_empty() {
                            continue;
                        }
                        let offset = begin + count * self.partition_size;
                        let v = &mut vectors[j];
                        let dim = book.dimensions;
                        if format == 0 {
                            let step = self.partition_size / dim;
                            for i in 0..step {
                                let Some(e) = book.vector(b) else { return };
                                for (k, x) in e.iter().enumerate() {
                                    v[offset + i + k * step] += x;
                                }
                            }
                        } else {
                            let mut i = 0;
                            while i < self.partition_size {
                                let Some(e) = book.vector(b) else { return };
                                for (k, x) in e.iter().enumerate() {
                                    if let Some(s) = v.get_mut(offset + i + k) {
                                        *s += x;
                                    }
                                }
                                i += dim;
                            }
                        }
                    }
                    count += 1;
                }
            }
        }
    }
}

struct Mapping {
    coupling: Vec<(usize, usize)>,
    mux: Vec<usize>,
    submap_floor: Vec<usize>,
    submap_residue: Vec<usize>,
}

struct Mode {
    long: bool,
    mapping: usize,
}

/// Decoded headers.
pub struct Setup {
    pub channels: usize,
    pub rate: u32,
    pub blocksizes: [usize; 2],
    books: Vec<Codebook>,
    floors: Vec<Floor1>,
    residues: Vec<Residue>,
    mappings: Vec<Mapping>,
    modes: Vec<Mode>,
}

impl Setup {
    /// The identification and setup header packets.
    pub fn read(identification: &[u8], setup: &[u8]) -> io::Result<Self> {
        if identification.len() < 30 || &identification[..7] != b"\x01vorbis" {
            return Err(invalid("not a Vorbis identification header"));
        }
        let mut b = Bits::new(&identification[7..]);
        if need(b.read(32))? != 0 {
            return Err(invalid("Vorbis version"));
        }
        let channels = need(b.read(8))? as usize;
        let rate = need(b.read(32))?;
        b.read(32);
        b.read(32);
        b.read(32);
        let b0 = 1usize << need(b.read(4))?;
        let b1 = 1usize << need(b.read(4))?;
        if channels == 0 || !(64..=8192).contains(&b0) || b1 < b0 || b1 > 8192 {
            return Err(invalid("Vorbis identification values"));
        }
        if setup.len() < 7 || &setup[..7] != b"\x05vorbis" {
            return Err(invalid("not a Vorbis setup header"));
        }
        let mut b = Bits::new(&setup[7..]);
        let books = (0..need(b.read(8))? + 1)
            .map(|_| Codebook::read(&mut b))
            .collect::<io::Result<Vec<_>>>()?;
        for _ in 0..need(b.read(6))? + 1 {
            if need(b.read(16))? != 0 {
                return Err(invalid("Vorbis time domain type"));
            }
        }
        let floors = (0..need(b.read(6))? + 1)
            .map(|_| match need(b.read(16))? {
                1 => Floor1::read(&mut b, books.len()),
                _ => Err(invalid("Vorbis floor type other than 1")),
            })
            .collect::<io::Result<Vec<_>>>()?;
        let residues = (0..need(b.read(6))? + 1)
            .map(|_| match need(b.read(16))? {
                k @ 0..=2 => Residue::read(&mut b, k as u16, books.len()),
                _ => Err(invalid("Vorbis residue type")),
            })
            .collect::<io::Result<Vec<_>>>()?;
        let mappings = (0..need(b.read(6))? + 1)
            .map(|_| {
                if need(b.read(16))? != 0 {
                    return Err(invalid("Vorbis mapping type"));
                }
                let submaps = if need(b.flag())? {
                    need(b.read(4))? as usize + 1
                } else {
                    1
                };
                let mut coupling = Vec::new();
                if need(b.flag())? {
                    let bits = ilog(channels as u32 - 1);
                    for _ in 0..need(b.read(8))? + 1 {
                        let m = need(b.read(bits))? as usize;
                        let a = need(b.read(bits))? as usize;
                        if m == a || m >= channels || a >= channels {
                            return Err(invalid("Vorbis coupling channels"));
                        }
                        coupling.push((m, a));
                    }
                }
                if need(b.read(2))? != 0 {
                    return Err(invalid("Vorbis mapping reserved"));
                }
                let mux = if submaps > 1 {
                    (0..channels)
                        .map(|_| need(b.read(4)).map(|v| v as usize))
                        .collect::<io::Result<Vec<_>>>()?
                } else {
                    vec![0; channels]
                };
                let mut submap_floor = Vec::new();
                let mut submap_residue = Vec::new();
                for _ in 0..submaps {
                    b.read(8);
                    let f = need(b.read(8))? as usize;
                    let r = need(b.read(8))? as usize;
                    if f >= floors.len() || r >= residues.len() {
                        return Err(invalid("Vorbis submap"));
                    }
                    submap_floor.push(f);
                    submap_residue.push(r);
                }
                Ok(Mapping {
                    coupling,
                    mux,
                    submap_floor,
                    submap_residue,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let modes = (0..need(b.read(6))? + 1)
            .map(|_| {
                let long = need(b.flag())?;
                b.read(16);
                b.read(16);
                let mapping = need(b.read(8))? as usize;
                if mapping >= mappings.len() {
                    return Err(invalid("Vorbis mode mapping"));
                }
                Ok(Mode { long, mapping })
            })
            .collect::<io::Result<Vec<_>>>()?;
        if !need(b.flag())? {
            return Err(invalid("Vorbis setup framing"));
        }
        Ok(Self {
            channels,
            rate,
            blocksizes: [b0, b1],
            books,
            floors,
            residues,
            mappings,
            modes,
        })
    }
}

/// The decoder state: the previous block's windowed second half.
pub struct Decoder {
    pub setup: Setup,
    previous: Option<(usize, Vec<Vec<f32>>)>,
}

impl Decoder {
    pub fn new(setup: Setup) -> Self {
        Self {
            setup,
            previous: None,
        }
    }

    /// Forget the overlap (a seek): the next packet outputs nothing.
    pub fn reset(&mut self) {
        self.previous = None;
    }

    /// Decodes one audio packet; returns per-channel samples (none for the
    /// first packet after a reset or for a non-audio packet).
    pub fn decode(&mut self, packet: &[u8]) -> io::Result<Vec<Vec<f32>>> {
        let s = &self.setup;
        let mut b = Bits::new(packet);
        if need(b.flag())? {
            return Ok(Vec::new());
        }
        let mode = &s.modes[need(b.read(ilog(s.modes.len() as u32 - 1)))? as usize];
        let n = s.blocksizes[mode.long as usize];
        let (prev_long, next_long) = if mode.long {
            (need(b.flag())?, need(b.flag())?)
        } else {
            (false, false)
        };
        let mapping = &s.mappings[mode.mapping];
        let half = n / 2;
        let ch = s.channels;
        // Floors.
        let mut floors: Vec<Option<Vec<i32>>> = Vec::with_capacity(ch);
        for c in 0..ch {
            let floor = &s.floors[mapping.submap_floor[mapping.mux[c]]];
            floors.push(floor.decode(&mut b, &s.books));
        }
        let mut used: Vec<bool> = floors.iter().map(|f| f.is_some()).collect();
        for &(m, a) in &mapping.coupling {
            if used[m] || used[a] {
                used[m] = true;
                used[a] = true;
            }
        }
        // Residues per submap.
        let mut residue = vec![vec![0f32; half]; ch];
        for (sub, &r) in mapping.submap_residue.iter().enumerate() {
            let chans: Vec<usize> = (0..ch).filter(|&c| mapping.mux[c] == sub).collect();
            let mut vecs: Vec<Vec<f32>> = chans.iter().map(|_| vec![0f32; half]).collect();
            let skip: Vec<bool> = chans.iter().map(|&c| !used[c]).collect();
            s.residues[r].decode(&mut b, &s.books, &mut vecs, &skip);
            for (k, &c) in chans.iter().enumerate() {
                residue[c] = std::mem::take(&mut vecs[k]);
            }
        }
        // Inverse coupling, last step first.
        for &(m, a) in mapping.coupling.iter().rev() {
            let mut mags = std::mem::take(&mut residue[m]);
            let mut angs = std::mem::take(&mut residue[a]);
            for (mr, ar) in mags.iter_mut().zip(angs.iter_mut()) {
                let (mv, av) = (*mr, *ar);
                let (nm, na) = if mv > 0. {
                    if av > 0. {
                        (mv, mv - av)
                    } else {
                        (mv + av, mv)
                    }
                } else if av > 0. {
                    (mv, mv + av)
                } else {
                    (mv - av, mv)
                };
                *mr = nm;
                *ar = na;
            }
            residue[m] = mags;
            residue[a] = angs;
        }
        // Floor x residue, inverse MDCT, window.
        let window = window(n, s.blocksizes, mode.long, prev_long, next_long);
        let mut blocks = Vec::with_capacity(ch);
        for c in 0..ch {
            let mut spectrum = vec![0f32; half];
            if let Some(y) = &floors[c] {
                let floor = &s.floors[mapping.submap_floor[mapping.mux[c]]];
                floor.synthesize(y, half, &mut spectrum);
                for (sv, r) in spectrum.iter_mut().zip(&residue[c]) {
                    *sv *= r;
                }
            }
            let mut out = imdct(&spectrum);
            for (o, w) in out.iter_mut().zip(&window) {
                *o *= w;
            }
            blocks.push(out);
        }
        // Overlap-add from the previous block's centre to this one's.
        let result = match self.previous.take() {
            None => Vec::new(),
            Some((pn, tails)) => (0..ch)
                .map(|c| {
                    let len = pn / 4 + n / 4;
                    (0..len)
                        .map(|k| {
                            let t = tails[c].get(k).copied().unwrap_or(0.);
                            let i = k as isize - (pn / 4) as isize + (n / 4) as isize;
                            let cur = if i >= 0 {
                                blocks[c].get(i as usize).copied().unwrap_or(0.)
                            } else {
                                0.
                            };
                            t + cur
                        })
                        .collect()
                })
                .collect(),
        };
        self.previous = Some((
            n,
            blocks.into_iter().map(|bl| bl[half..].to_vec()).collect(),
        ));
        Ok(result)
    }
}

/// The Vorbis window for a block of size `n` (slopes sized by the
/// neighbouring blocks).
fn window(n: usize, sizes: [usize; 2], long: bool, prev_long: bool, next_long: bool) -> Vec<f32> {
    let short = sizes[0];
    let (lb, ln) = if long && !prev_long {
        (n / 4 - short / 4, short / 2)
    } else {
        (0, n / 2)
    };
    let (rb, rn) = if long && !next_long {
        (n * 3 / 4 - short / 4, short / 2)
    } else {
        (n / 2, n / 2)
    };
    let slope = |i: usize, len: usize| {
        let x = (i as f64 + 0.5) / len as f64 * std::f64::consts::FRAC_PI_2;
        (std::f64::consts::FRAC_PI_2 * x.sin().powi(2)).sin() as f32
    };
    (0..n)
        .map(|i| {
            if i < lb {
                0.
            } else if i < lb + ln {
                slope(i - lb, ln)
            } else if i < rb {
                1.
            } else if i < rb + rn {
                slope(rb + rn - 1 - i, rn)
            } else {
                0.
            }
        })
        .collect()
}

/// Inverse MDCT of n/2 coefficients into n samples:
/// y[i] = sum_k X[k] cos(2pi/n (i + 1/2 + n/4)(k + 1/2)), through a DCT-IV
/// computed with an n/4-point complex FFT.
pub fn imdct(x: &[f32]) -> Vec<f32> {
    let m = x.len();
    let u = dct4(x);
    let mut y = vec![0f32; 2 * m];
    for (i, o) in y.iter_mut().enumerate() {
        *o = if i < m / 2 {
            u[i + m / 2]
        } else if i < 3 * m / 2 {
            -u[3 * m / 2 - 1 - i]
        } else {
            -u[i - 3 * m / 2]
        };
    }
    y
}

/// DCT-IV: u[n] = sum_k x[k] cos(pi/M (n + 1/2)(k + 1/2)).
fn dct4(x: &[f32]) -> Vec<f32> {
    let m = x.len();
    let h = m / 2;
    let pi = std::f64::consts::PI;
    // t[n] = (x[2n] + i x[M-1-2n]) * exp(-i pi (4n + 1) / (4M))
    let mut re = vec![0f64; h];
    let mut im = vec![0f64; h];
    for n in 0..h {
        let (a, b) = (x[2 * n] as f64, x[m - 1 - 2 * n] as f64);
        let ang = -pi * (4 * n + 1) as f64 / (4 * m) as f64;
        let (s, c) = ang.sin_cos();
        re[n] = a * c - b * s;
        im[n] = a * s + b * c;
    }
    fft(&mut re, &mut im);
    let mut u = vec![0f32; m];
    for k in 0..h {
        let ang = -pi * k as f64 / m as f64;
        let (s, c) = ang.sin_cos();
        let (r, i) = (re[k] * c - im[k] * s, re[k] * s + im[k] * c);
        u[2 * k] = r as f32;
        u[m - 1 - 2 * k] = -i as f32;
    }
    u
}

/// In-place radix-2 complex FFT (forward, exp(-2 pi i nk / N)).
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    if n <= 1 {
        return;
    }
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2. * std::f64::consts::PI / len as f64;
        let (ws, wc) = ang.sin_cos();
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1f64, 0f64);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let nr = cr * wc - ci * ws;
                ci = cr * ws + ci * wc;
                cr = nr;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imdct_matches_the_definition() {
        for n in [16usize, 64, 256] {
            let x: Vec<f32> = (0..n / 2)
                .map(|k| ((k * 37 % 11) as f32 - 5.) * 0.25)
                .collect();
            let fast = imdct(&x);
            for (i, &f) in fast.iter().enumerate() {
                let slow: f64 = x
                    .iter()
                    .enumerate()
                    .map(|(k, &v)| {
                        v as f64
                            * (2. * std::f64::consts::PI / n as f64
                                * (i as f64 + 0.5 + n as f64 / 4.)
                                * (k as f64 + 0.5))
                                .cos()
                    })
                    .sum();
                assert!((f as f64 - slow).abs() < 1e-3, "n {n} i {i}: {f} vs {slow}");
            }
        }
    }

    #[test]
    fn canonical_codewords_follow_the_spec_example() {
        // Spec example: lengths 2,4,4,4,4,2,3,3 -> 00, 0100, 0101, 0110,
        // 0111, 10, 110, 111.
        let tree = build_tree(&[2, 4, 4, 4, 4, 2, 3, 3]).unwrap();
        let book = Codebook {
            dimensions: 1,
            tree,
            vectors: Vec::new(),
        };
        // Codes written MSB first into an LSB-first stream.
        let codes: [(u32, u32); 8] = [
            (0b00, 2),
            (0b0100, 4),
            (0b0101, 4),
            (0b0110, 4),
            (0b0111, 4),
            (0b10, 2),
            (0b110, 3),
            (0b111, 3),
        ];
        let mut bits = Vec::new();
        for &(c, l) in &codes {
            for k in (0..l).rev() {
                bits.push((c >> k) & 1);
            }
        }
        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (i, b) in bits.iter().enumerate() {
            bytes[i / 8] |= (*b as u8) << (i % 8);
        }
        let mut r = Bits::new(&bytes);
        let got: Vec<usize> = (0..8).map(|_| book.scalar(&mut r).unwrap()).collect();
        assert_eq!(got, (0..8).collect::<Vec<_>>());
    }

    #[test]
    fn small_helpers() {
        assert_eq!(ilog(0), 0);
        assert_eq!(ilog(7), 3);
        assert_eq!(lookup1_values(625, 4), 5);
        assert_eq!(float32_unpack(0x6080_0001), 2f32.powi(772 - 788));
        assert!((inverse_db(255) - 1.).abs() < 1e-6);
        let w = window(256, [256, 2048], false, false, false);
        // Power complementary halves.
        for i in 0..128 {
            assert!((w[i] * w[i] + w[i + 128] * w[i + 128] - 1.).abs() < 1e-5);
        }
    }
}
