//! TU3 APT action bytecode, read without running it
//! (tools/vendor/skate3_ui/actions.py). CONSTANTPOOL/PUSHDATA use CONST
//! indices; operands are 4-byte aligned.
use super::binary::{Reader, align, bad};
use super::json::{J, Obj};
use crate::Error;

const BYTE: [u8; 9] = [0xA2, 0xAE, 0xAF, 0xB0, 0xB1, 0xB2, 0xB3, 0xB5, 0xB9];
const SHORT: [u8; 2] = [0xA3, 0xB6];
const WORD: [u8; 6] = [0x81, 0x87, 0x99, 0x9D, 0x9F, 0xB8];
const STRING: [u8; 7] = [0x8B, 0x8C, 0xA1, 0xA4, 0xA5, 0xA6, 0xA7];

pub struct Actions<'a> {
    apt: Reader<'a>,
    constants: Vec<J>,
}

impl<'a> Actions<'a> {
    pub fn new(apt: &'a [u8], constants: &[u8]) -> Result<Self, Error> {
        let c = Reader::new(constants);
        let (count, start) = (c.at(24)?, c.at(28)?);
        c.require(start, count * 8)?;
        let mut table = Vec::with_capacity(count);
        for i in 0..count {
            let at = start + i * 8;
            let (kind, raw) = (c.u32be(at)?, c.u32be(at + 4)?);
            let value = match kind {
                1 => {
                    c.require(raw as usize, 1)?;
                    J::Str(c.cstring(raw as usize))
                }
                6 => J::Float(c.f32be(at + 4)?),
                7 => J::from(c.i32be(at + 4)?),
                _ => J::from(raw),
            };
            let mut constant = Obj::new();
            constant.set("kind", kind);
            constant.set("value", value);
            table.push(J::Obj(constant));
        }
        Ok(Actions {
            apt: Reader::new(apt),
            constants: table,
        })
    }

    /// The instructions from `offset` to the first end opcode (or `end`).
    pub fn stream(
        &self,
        mut offset: usize,
        end: Option<usize>,
        nesting: u32,
    ) -> Result<Vec<J>, Error> {
        if nesting > 64 {
            return Err(bad("APT action nesting exceeds 64"));
        }
        let r = &self.apt;
        let end = end.unwrap_or(r.data.len());
        if end < offset {
            return Err(bad("APT action range is negative"));
        }
        r.require(offset, end - offset)?;
        let mut result = Vec::new();
        while offset < end {
            let (at, op) = (offset, r.u8(offset)?);
            offset += 1;
            let mut row = Obj::new();
            row.set("offset", at);
            row.set("opcode", op);
            if op == 0 {
                result.push(J::Obj(row));
                break;
            }
            if BYTE.contains(&op) {
                row.set("operand", r.u8(offset)?);
                offset += 1;
            } else if SHORT.contains(&op) {
                row.set("operand", r.u16be(offset)?);
                offset += 2;
            } else if [0x77, 0xB4, 0xB7].contains(&op) {
                row.set("operand", r.i32be(offset)?);
                offset += 4;
            } else if WORD.contains(&op) || STRING.contains(&op) || op == 0x94 {
                offset = align(offset, 4);
                let value = r.u32be(offset)?;
                if STRING.contains(&op) {
                    row.set("operand", r.cstring(value as usize));
                } else {
                    row.set("operand", value);
                }
                if [0x99, 0x9D, 0x94, 0xB8].contains(&op) {
                    row.set("target", offset as i64 + 4 + r.i32be(offset)? as i64);
                }
                offset += 4;
            } else if op == 0x88 || op == 0x96 {
                offset = align(offset, 4);
                let (count, pointer) = (r.at(offset)?, r.at(offset + 4)?);
                r.require(pointer, count * 4)?;
                let mut values = Vec::with_capacity(count);
                for i in 0..count {
                    let index = r.at(pointer + i * 4)?;
                    values.push(index);
                }
                if values.iter().any(|&i| i >= self.constants.len()) {
                    return Err(bad("APT constant index outside CONST"));
                }
                row.set(
                    "values",
                    values
                        .into_iter()
                        .map(|i| self.constants[i].clone())
                        .collect::<Vec<_>>(),
                );
                offset += 8;
            } else if op == 0x8E || op == 0x9B {
                offset = align(offset, 4);
                let define2 = op == 0x8E;
                let header_size = if define2 { 28 } else { 24 };
                r.require(offset, header_size)?;
                row.set("name", r.cstring(r.at(offset)?));
                let count = r.at(offset + 4)?;
                let pointer = r.at(offset + if define2 { 12 } else { 8 })?;
                let length = r.at(offset + if define2 { 16 } else { 12 })?;
                let stride = if define2 { 8 } else { 4 };
                r.require(pointer, count * stride)?;
                let mut parameters = Vec::with_capacity(count);
                for i in 0..count {
                    let mut p = Obj::new();
                    p.set(
                        "register",
                        if define2 {
                            r.u32be(pointer + i * 8)?
                        } else {
                            0
                        },
                    );
                    let name_at = pointer + i * stride + if define2 { 4 } else { 0 };
                    p.set("name", r.cstring(r.at(name_at)?));
                    parameters.push(J::Obj(p));
                }
                row.set("parameters", parameters);
                row.set("flags", if define2 { r.u32be(offset + 8)? } else { 0 });
                offset += header_size;
                row.set(
                    "body",
                    self.stream(offset, Some(offset + length), nesting + 1)?,
                );
                offset += length;
            } else if op == 0x83 || op == 0x8F {
                // Unsupported compound instructions are never scanned as plain ops.
                return Err(bad("APT compound opcode needs decoding"));
            }
            row.set("next", offset);
            if offset > end {
                return Err(bad("APT instruction crosses its function body"));
            }
            result.push(J::Obj(row));
        }
        Ok(result)
    }
}
