//! GameLZSS.Compress port, including hash-chain order and overlapping matches.
struct Window {
    bytes: [u8; 4096],
    head: Vec<i32>,
    next: Vec<i32>,
    prev: Vec<i32>,
    hashes: Vec<i32>,
}
impl Window {
    fn new() -> Self {
        let mut w = Self {
            bytes: [0x20; 4096],
            head: vec![-1; 65536],
            next: vec![-1; 4096],
            prev: vec![-1; 4096],
            hashes: vec![-1; 4096],
        };
        w.bytes[4078..].fill(0);
        for i in 0..4096 {
            w.insert(i);
        }
        w
    }
    fn hash(a: u8, b: u8, c: u8) -> usize {
        (((a as usize) << 8) ^ ((b as usize) << 4) ^ c as usize) & 65535
    }
    fn insert(&mut self, i: usize) {
        let h = Self::hash(
            self.bytes[i],
            self.bytes[(i + 1) & 4095],
            self.bytes[(i + 2) & 4095],
        );
        self.next[i] = self.head[h];
        self.prev[i] = -1;
        if self.head[h] != -1 {
            self.prev[self.head[h] as usize] = i as i32;
        }
        self.head[h] = i as i32;
        self.hashes[i] = h as i32;
    }
    fn remove(&mut self, i: usize) {
        if self.hashes[i] == -1 {
            return;
        }
        let p = self.prev[i];
        let n = self.next[i];
        if p != -1 {
            self.next[p as usize] = n;
        } else {
            self.head[self.hashes[i] as usize] = n;
        }
        if n != -1 {
            self.prev[n as usize] = p;
        }
        self.hashes[i] = -1;
        self.next[i] = -1;
        self.prev[i] = -1;
    }
    fn update(&mut self, i: usize, b: u8) {
        let indices = [(i + 4094) & 4095, (i + 4095) & 4095, i];
        for p in indices {
            self.remove(p);
        }
        self.bytes[i] = b;
        for p in indices {
            self.insert(p);
        }
    }
    fn match_len(
        &self,
        input: &[u8],
        src: usize,
        i: usize,
        dist: usize,
        start: usize,
        limit: usize,
    ) -> usize {
        let mut m = start;
        while m < limit {
            let expected = if m < dist {
                self.bytes[(i + m) & 4095]
            } else {
                input[src + m % dist]
            };
            if expected != input[src + m] {
                break;
            }
            m += 1;
        }
        m
    }
}
pub(crate) fn compress(input: &[u8], header: &[u8]) -> Vec<u8> {
    let mut out = header[..64].to_vec();
    out[16..20].copy_from_slice(&(input.len() as u32).to_le_bytes());
    let mut w = Window::new();
    let mut r = 4078usize;
    let mut src = 0;
    while src < input.len() {
        let flags = out.len();
        out.push(0);
        for bit in 0..8 {
            if src == input.len() {
                break;
            }
            let limit = 18.min(input.len() - src);
            let mut key = 0;
            let mut length = 0;
            if limit >= 3 {
                let h = Window::hash(input[src], input[src + 1], input[src + 2]);
                let mut i = w.head[h];
                let mut depth = 0;
                while i != -1 && depth < 256 {
                    depth += 1;
                    let pos = i as usize;
                    let mut dist = (r + 4096 - pos) & 4095;
                    if dist == 0 {
                        dist = 4096;
                    }
                    if dist >= 3
                        && w.bytes[pos] == input[src]
                        && w.bytes[(pos + 1) & 4095] == input[src + 1]
                        && w.bytes[(pos + 2) & 4095] == input[src + 2]
                    {
                        let m = w.match_len(input, src, pos, dist, 3, limit);
                        if m > length {
                            key = pos;
                            length = m;
                            if length == limit {
                                break;
                            }
                        }
                    }
                    i = w.next[pos];
                }
                if length < limit {
                    for dist in 1..=2 {
                        let pos = (r + 4096 - dist) & 4095;
                        if w.bytes[pos] == input[src] {
                            let m = w.match_len(input, src, pos, dist, 1, limit);
                            if m > length {
                                key = pos;
                                length = m;
                                if length == limit {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if length <= 2 {
                length = 1;
                out[flags] |= 1 << bit;
                out.push(input[src]);
            } else {
                out.push((key & 255) as u8);
                out.push((((key >> 4) & 240) | (length - 3)) as u8);
            }
            for k in 0..length {
                w.update(r, input[src + k]);
                r = (r + 1) & 4095;
            }
            src += length;
        }
    }
    out
}
