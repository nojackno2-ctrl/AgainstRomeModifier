use crate::Result;
#[derive(Clone, Debug, PartialEq)]
pub struct BciImage {
    pub code: Vec<i32>,
    pub symbols: Vec<u8>,
    pub constants: Vec<u8>,
    pub constant_offsets: Vec<i32>,
    pub variables: Vec<u8>,
    pub variable_offsets: Vec<i32>,
    pub dtor: i32,
    pub main: i32,
}
impl BciImage {
    pub fn idle() -> Self {
        Self {
            code: vec![95, 75, 66, 0, 87, 130, 74, 94, 73, 0, 66, 10, 131, 112, -20],
            symbols: vec![],
            constants: vec![],
            constant_offsets: vec![],
            variables: vec![],
            variable_offsets: vec![],
            dtor: 0,
            main: 24,
        }
    }
    pub fn parse(data: &[u8]) -> Result<Self> {
        struct Reader<'a> {
            d: &'a [u8],
            p: usize,
        }
        impl<'a> Reader<'a> {
            fn take(&mut self, n: usize) -> Result<&'a [u8]> {
                let end = self.p.checked_add(n).ok_or("BCI length overflow")?;
                let b = self.d.get(self.p..end).ok_or("truncated BCI")?;
                self.p = end;
                Ok(b)
            }
            fn word(&mut self) -> Result<i32> {
                Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
            }
            fn tag(&mut self, t: &[u8]) -> Result<()> {
                if self.take(4)? != t {
                    Err("bad BCI section tag".into())
                } else {
                    Ok(())
                }
            }
        }
        let mut r = Reader { d: data, p: 0 };
        r.tag(b"BCI0")?;
        if r.word()? != 1 {
            return Err("unsupported BCI version".into());
        }
        let mut lens = [0usize; 6];
        for l in &mut lens {
            *l = usize::try_from(r.word()?).map_err(|_| "negative BCI length")?;
            if *l > data.len() {
                return Err("BCI length exceeds file".into());
            }
        }
        r.tag(b"CODE")?;
        let raw = r.take(lens[0])?;
        if raw.len() % 4 != 0 {
            return Err("unaligned CODE".into());
        }
        let code = raw
            .chunks_exact(4)
            .map(|x| i32::from_le_bytes(x.try_into().unwrap()))
            .collect();
        r.tag(b"SYMB")?;
        let symbols = r.take(lens[1])?.to_vec();
        r.tag(b"CONS")?;
        let constants = r.take(lens[2])?.to_vec();
        r.tag(b"CIDX")?;
        let mut constant_offsets = vec![];
        for _ in 0..lens[3] {
            let v = r.word()?;
            if v < 0 || v as usize >= constants.len() {
                return Err("invalid constant offset".into());
            }
            if !constants[v as usize..].contains(&0) {
                return Err("unterminated BCI constant".into());
            }
            constant_offsets.push(v)
        }
        r.tag(b"VAR ")?;
        let variables = r.take(lens[4])?.to_vec();
        r.tag(b"VIDX")?;
        let mut variable_offsets = vec![];
        for _ in 0..lens[5] {
            variable_offsets.push(r.word()?)
        }
        let dtor = r.word()?;
        let main = r.word()?;
        if r.p != data.len()
            || [main, dtor]
                .iter()
                .any(|x| *x < 0 || *x as usize >= lens[0] || x % 4 != 0)
        {
            return Err("bad BCI entrypoint or trailing bytes".into());
        }
        Ok(Self {
            code,
            symbols,
            constants,
            constant_offsets,
            variables,
            variable_offsets,
            dtor,
            main,
        })
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut o = b"BCI0".to_vec();
        fn w(o: &mut Vec<u8>, v: i32) {
            o.extend(v.to_le_bytes())
        }
        w(&mut o, 1);
        for n in [
            self.code.len() * 4,
            self.symbols.len(),
            self.constants.len(),
            self.constant_offsets.len(),
            self.variables.len(),
            self.variable_offsets.len(),
        ] {
            w(&mut o, n as i32)
        }
        o.extend(b"CODE");
        for &v in &self.code {
            w(&mut o, v)
        }
        o.extend(b"SYMB");
        o.extend(&self.symbols);
        o.extend(b"CONS");
        o.extend(&self.constants);
        o.extend(b"CIDX");
        for &v in &self.constant_offsets {
            w(&mut o, v)
        }
        o.extend(b"VAR ");
        o.extend(&self.variables);
        o.extend(b"VIDX");
        for &v in &self.variable_offsets {
            w(&mut o, v)
        }
        w(&mut o, self.dtor);
        w(&mut o, self.main);
        o
    }
    pub fn constant(&self, i: usize) -> Result<&[u8]> {
        let s = *self.constant_offsets.get(i).ok_or("bad constant index")? as usize;
        let b = self.constants.get(s..).ok_or("bad constant offset")?;
        let end = b
            .iter()
            .position(|x| *x == 0)
            .ok_or("unterminated BCI constant")?;
        Ok(&b[..end])
    }
    pub fn add_bytes(&mut self, b: &[u8]) -> Result<i32> {
        if b.contains(&0) {
            return Err("constant contains NUL".into());
        }
        let i = self.constant_offsets.len() as i32;
        self.constant_offsets.push(self.constants.len() as i32);
        self.constants.extend(b);
        self.constants.push(0);
        Ok(i)
    }
    pub fn add(&mut self, s: &str) -> Result<i32> {
        let bytes: Vec<u8> = s
            .chars()
            .map(|c| {
                u8::try_from(c as u32).map_err(|_| "native identifier must be Latin-1".to_owned())
            })
            .collect::<Result<_>>()?;
        self.add_bytes(&bytes)
    }
    pub fn native(&mut self, s: &str) -> Result<i32> {
        for i in 0..self.constant_offsets.len() {
            if self.constant(i)? == s.as_bytes() {
                return Ok(i as i32);
            }
        }
        self.add(s)
    }
    pub fn game_string(&mut self, s: &str) -> Result<i32> {
        let (b, _, bad) = encoding_rs::WINDOWS_1251.encode(s);
        if bad {
            return Err("message not representable in Windows-1251".into());
        }
        self.add_bytes(&b)
    }
    pub fn append(&mut self, w: &[i32]) -> i32 {
        let a = self.code.len() as i32 * 4;
        self.code.extend(w);
        a
    }
}
