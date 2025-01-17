use nohash_hasher::NoHashHasher;
use rand::seq::SliceRandom;
use std::{collections::HashMap, hash::BuildHasherDefault};
extern crate rand;

#[derive(Clone, Copy)]
pub enum AddressingMode {
    Implicit,
    Accumulator,
    Immediate(i8),
    Absolute(u16),
}

#[derive(Clone, Copy)]
pub struct Instruction {
    opname: &'static str,
    pub operation: fn(&Instruction, &mut State) -> bool,
    src: AddressingMode,
    dst: AddressingMode,
}

pub fn add_to_reg8(
    reg: Option<i8>,
    a: Option<i8>,
) -> (
    Option<i8>,
    Option<bool>,
    Option<bool>,
    Option<bool>,
    Option<bool>,
    Option<bool>,
) {
    // The return values are the result of the addition, then the flags, carry, zero, sign, overflow, half-carry.
    if let Some(v) = a {
        if let Some(r) = reg {
            let result = r.wrapping_add(v);
            let z = result == 0;
            let c = r.checked_add(v).is_none();
            let n = result < 0;
            let o = (r < 0 && v < 0 && result >= 0) || (r > 0 && v > 0 && result <= 0);
            let h = ((r ^ v ^ result) & 0x10) == 0x10;
            (Some(result), Some(c), Some(z), Some(n), Some(o), Some(h))
        } else {
            (None, None, None, None, None, None)
        }
    } else {
        (None, None, None, None, None, None)
    }
}

fn decimal_adjust(
    accumulator: Option<i8>,
    carry: Option<bool>,
    halfcarry: Option<bool>,
) -> Option<i8> {
    fn nybble(val: i8, flag: Option<bool>) -> Option<i8> {
        if val & 0x0f > 0x09 {
            return Some(0x06);
        }
        flag?;
        if flag.unwrap_or(false) {
            return Some(0x06);
        }
        Some(0)
    }

    if let Some(a) = accumulator {
        if let Some(right) = nybble(a, halfcarry) {
            let ar = a + right;
            nybble(ar >> 4, carry).map(|left| ar + (left << 4))
        } else {
            None
        }
    } else {
        None
    }
}

fn rotate_left_thru_carry(val: Option<i8>, carry: Option<bool>) -> (Option<i8>, Option<bool>) {
    if val.is_none() || carry.is_none() {
        (None, None)
    } else {
        let c = carry.unwrap();
        let v = val.unwrap();
        let high_bit_set = v & -128 != 0;
        let shifted = (v & 0x7f).rotate_left(1);
        (
            Some(if c { shifted + 1 } else { shifted }),
            Some(high_bit_set),
        )
    }
}

fn rotate_right_thru_carry(val: Option<i8>, carry: Option<bool>) -> (Option<i8>, Option<bool>) {
    if val.is_none() || carry.is_none() {
        (None, None)
    } else {
        let c = carry.unwrap();
        let v = val.unwrap();
        let low_bit_set = v & 1 != 0;
        let shifted = (v & 0x7f).rotate_right(1);
        (
            Some(if c { shifted | -128i8 } else { shifted }),
            Some(low_bit_set),
        )
    }
}

#[test]
fn add_to_reg8_test() {
    assert_eq!(
        add_to_reg8(Some(3), 3),
        (Some(6), Some(false), Some(false), Some(false), Some(false))
    );
    assert_eq!(
        add_to_reg8(Some(127), 1),
        (Some(-128), Some(true), Some(false), Some(true), Some(false))
    );
    assert_eq!(add_to_reg8(None, 3), (None, None, None, None, None));
}

impl Instruction {
    pub fn inh(
        opname: &'static str,
        operation: for<'r, 's> fn(&'r Instruction, &'s mut State) -> bool,
    ) -> Instruction {
        Instruction {
            opname,
            operation,
            src: AddressingMode::Implicit,
            dst: AddressingMode::Implicit,
        }
    }

    pub fn imm(
        opname: &'static str,
        operation: for<'r, 's> fn(&'r Instruction, &'s mut State) -> bool,
    ) -> Instruction {
        Instruction {
            opname,
            operation,
            src: AddressingMode::Immediate(0),
            dst: AddressingMode::Immediate(0),
        }
    }

    pub fn abs(
        opname: &'static str,
        operation: for<'r, 's> fn(&'r Instruction, &'s mut State) -> bool,
    ) -> Instruction {
        Instruction {
            opname,
            operation,
            src: AddressingMode::Absolute(0),
            dst: AddressingMode::Absolute(0),
        }
    }

    pub fn randomize(&mut self, constants: Vec<i8>, vars: Vec<u16>) {
        match self.src {
            AddressingMode::Implicit => {
                self.src = AddressingMode::Implicit;
            }
            AddressingMode::Accumulator => {
                self.src = AddressingMode::Accumulator;
            }
            AddressingMode::Immediate(_) => {
                if let Some(r) = constants.choose(&mut rand::thread_rng()) {
                    // If there's any constants, then pick one.
                    self.src = AddressingMode::Immediate(*r);
                } else {
                    // Otherwise pick any i8.
                    self.src = AddressingMode::Immediate(rand::random());
                }
            }
            AddressingMode::Absolute(_) => {
                if let Some(r) = vars.choose(&mut rand::thread_rng()) {
                    // If there's any variables, then pick one.
                    self.src = AddressingMode::Absolute(*r);
                } else {
                    // Otherwise pick any random address. (this is unlikely to be any good)
                    self.src = AddressingMode::Absolute(rand::random());
                }
            }
        }
    }

    pub fn vectorize(&self, prog: &mut Vec<Instruction>, constants: &[i8], vars: &[u16]) {
        match self.src {
            AddressingMode::Implicit | AddressingMode::Accumulator => prog.push(*self),
            AddressingMode::Immediate(_) => prog.extend(constants.iter().map(|c| Instruction {
                opname: self.opname,
                operation: self.operation,
                src: AddressingMode::Immediate(*c),
                dst: AddressingMode::Immediate(*c),
            })),
            AddressingMode::Absolute(_) => prog.extend(vars.iter().map(|c| Instruction {
                opname: self.opname,
                operation: self.operation,
                src: AddressingMode::Absolute(*c),
                dst: AddressingMode::Absolute(*c),
            })),
        }
    }

    fn get_datum(&self, m: &State) -> Option<i8> {
        match self.src {
            AddressingMode::Implicit => {
                panic!();
            }
            AddressingMode::Accumulator => m.accumulator,
            AddressingMode::Immediate(constant) => Some(constant),
            AddressingMode::Absolute(address) => m.heap.get(&address).copied()?,
        }
    }

    fn write_datum(&self, m: &mut State, val: Option<i8>) {
        match self.dst {
            AddressingMode::Implicit => {
                panic!();
            }
            AddressingMode::Accumulator => {
                m.accumulator = val;
            }
            AddressingMode::Immediate(_) => {
                panic!();
            }
            AddressingMode::Absolute(address) => {
                m.heap.insert(address, val);
            }
        }
    }

    fn op_aba(&self, s: &mut State) -> bool {
        let (result, c, z, n, o, h) = add_to_reg8(s.accumulator, s.reg_b);
        s.accumulator = result;
        s.sign = n;
        s.carry = c;
        s.zero = z;
        s.overflow = o;
        s.halfcarry = h;
        true
    }

    fn op_add(&self, s: &mut State) -> bool {
        let (result, c, z, n, o, h) = add_to_reg8(s.accumulator, self.get_datum(s));
        s.accumulator = result;
        s.sign = n;
        s.carry = c;
        s.zero = z;
        s.overflow = o;
        s.halfcarry = h;
        true
    }

    fn op_asl(&self, s: &mut State) -> bool {
        let (val, c) = rotate_left_thru_carry(s.accumulator, Some(false));
        s.accumulator = val;
        s.carry = c;
        true
    }

    fn op_adc(&self, s: &mut State) -> bool {
        let (result, c, z, n, o, h) = add_to_reg8(s.accumulator, self.get_datum(s));
        s.accumulator = result;
        s.sign = n;
        s.carry = c;
        s.zero = z;
        s.overflow = o;
        s.halfcarry = h;
        true
    }

    fn op_adc_dp(&self, s: &mut State) -> bool {
        // TODO: Check decimal flag here.
        let (result, c, z, n, o, h) = add_to_reg8(s.accumulator, self.get_datum(s));
        s.accumulator = result;
        s.sign = n;
        s.carry = c;
        s.zero = z;
        s.overflow = o;
        s.halfcarry = h;
        true
    }

    fn op_clc(&self, s: &mut State) -> bool {
        s.carry = Some(false);
        true
    }

    fn op_dea(&self, s: &mut State) -> bool {
        let (result, _c, z, n, _o, _h) = add_to_reg8(s.accumulator, Some(-1));
        s.accumulator = result;
        s.zero = z;
        s.sign = n;
        true
    }

    fn op_dex(&self, s: &mut State) -> bool {
        let (result, _c, z, n, _o, _h) = add_to_reg8(s.x8, Some(-1));
        s.x8 = result;
        s.zero = z;
        s.sign = n;
        true
    }

    fn op_dey(&self, s: &mut State) -> bool {
        let (result, _c, z, n, _o, _h) = add_to_reg8(s.y8, Some(-1));
        s.y8 = result;
        s.zero = z;
        s.sign = n;
        true
    }

    fn op_ina(&self, s: &mut State) -> bool {
        let (result, _c, z, n, _o, _h) = add_to_reg8(s.accumulator, Some(1));
        s.accumulator = result;
        s.zero = z;
        s.sign = n;
        true
    }

    fn op_inx(&self, s: &mut State) -> bool {
        let (result, _c, z, n, _o, _h) = add_to_reg8(s.x8, Some(1));
        s.x8 = result;
        s.zero = z;
        s.sign = n;
        true
    }

    fn op_iny(&self, s: &mut State) -> bool {
        let (result, _c, z, n, _o, _h) = add_to_reg8(s.y8, Some(1));
        s.y8 = result;
        s.zero = z;
        s.sign = n;
        true
    }

    fn op_lda(&self, s: &mut State) -> bool {
        s.accumulator = self.get_datum(s);
        true
    }

    fn op_lsr(&self, s: &mut State) -> bool {
        let (val, c) = rotate_right_thru_carry(s.accumulator, Some(false));
        s.accumulator = val;
        s.carry = c;
        true
    }

    fn op_rol(&self, s: &mut State) -> bool {
        let (val, c) = rotate_left_thru_carry(s.accumulator, s.carry);
        s.accumulator = val;
        s.carry = c;
        true
    }

    fn op_ror(&self, s: &mut State) -> bool {
        let (val, c) = rotate_right_thru_carry(s.accumulator, s.carry);
        s.accumulator = val;
        s.carry = c;
        true
    }

    fn op_sec(&self, s: &mut State) -> bool {
        s.carry = Some(true);
        true
    }

    fn op_sta(&self, s: &mut State) -> bool {
        self.write_datum(s, s.accumulator);
        true
    }

    fn op_stx(&self, s: &mut State) -> bool {
        self.write_datum(s, s.x8);
        true
    }

    fn op_sty(&self, s: &mut State) -> bool {
        self.write_datum(s, s.y8);
        true
    }

    fn op_stz(&self, s: &mut State) -> bool {
        self.write_datum(s, Some(0));
        true
    }

    fn op_tab(&self, s: &mut State) -> bool {
        // TODO: We need to check if this instruction affects flags or not,
        // I feel like this is an oversight
        s.reg_b = s.accumulator;
        true
    }

    fn op_tax(&self, s: &mut State) -> bool {
        // TODO: This one definitely needs flags.
        s.x8 = s.accumulator;
        true
    }

    fn op_tay(&self, s: &mut State) -> bool {
        // TODO: This one definitely needs flags.
        s.y8 = s.accumulator;
        true
    }

    fn op_tba(&self, s: &mut State) -> bool {
        // TODO: We need to check if this instruction affects flags or not,
        // I feel like this is an oversight
        s.accumulator = s.reg_b;
        true
    }

    fn op_txa(&self, s: &mut State) -> bool {
        // TODO: This one definitely needs flags.
        s.accumulator = s.x8;
        true
    }

    fn op_tya(&self, s: &mut State) -> bool {
        // TODO: This one definitely needs flags.
        s.accumulator = s.y8;
        true
    }
}

impl std::fmt::Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.src {
            AddressingMode::Implicit => {
                write!(f, "\t{}", self.opname)
            }
            AddressingMode::Accumulator => {
                write!(f, "\t{} a", self.opname)
            }
            AddressingMode::Immediate(constant) => {
                write!(f, "\t{} #{}", self.opname, constant)
            }
            AddressingMode::Absolute(address) => {
                write!(f, "\t{} {}", self.opname, address)
            }
        }
    }
}

//#[derive(Copy, Clone)]
pub struct State {
    accumulator: Option<i8>,
    reg_b: Option<i8>,
    x8: Option<i8>,
    y8: Option<i8>,
    zero: Option<bool>,
    carry: Option<bool>,
    sign: Option<bool>,
    decimal: Option<bool>,
    overflow: Option<bool>,
    halfcarry: Option<bool>,
    heap: HashMap<u16, Option<i8>, BuildHasherDefault<NoHashHasher<u16>>>,
}

impl State {
    pub fn new() -> State {
        State {
            accumulator: None,
            reg_b: None,
            x8: None,
            y8: None,
            zero: None,
            carry: None,
            sign: None,
            decimal: None,
            overflow: None,
            halfcarry: None,
            heap: HashMap::with_hasher(BuildHasherDefault::default()),
        }
    }
}

pub fn set_a(state: &mut State, a: i8) {
    state.accumulator = Some(a);
}
pub fn get_a(state: &State) -> Option<i8> {
    state.accumulator
}

pub fn set_b(state: &mut State, b: i8) {
    state.reg_b = Some(b);
}
pub fn get_b(state: &State) -> Option<i8> {
    state.reg_b
}

pub fn set_x(state: &mut State, x: i8) {
    state.x8 = Some(x);
}
pub fn get_x(state: &State) -> Option<i8> {
    state.x8
}

pub fn set_y(state: &mut State, y: i8) {
    state.y8 = Some(y);
}
pub fn get_y(state: &State) -> Option<i8> {
    state.y8
}

pub fn motorola6800() -> Vec<Instruction> {
    vec![
        Instruction::inh("aba", Instruction::op_aba),
        Instruction::imm("add", Instruction::op_add),
        Instruction::imm("adc", Instruction::op_adc),
        Instruction::inh("asla", Instruction::op_asl),
        Instruction::inh("tab", Instruction::op_tab),
        Instruction::inh("tba", Instruction::op_tba),
        Instruction::inh("rol", Instruction::op_rol),
        Instruction::inh("ror", Instruction::op_ror),
        Instruction::inh("clc", Instruction::op_clc),
        Instruction::inh("sec", Instruction::op_sec),
    ]
}

pub fn mos6502() -> Vec<Instruction> {
    vec![
        // TODO: Maybe we should have only one INC instruction, which can randomly go to either X or Y or the other possibilities.
        Instruction::inh("inx", Instruction::op_inx),
        Instruction::inh("iny", Instruction::op_iny),
        Instruction::inh("dex", Instruction::op_dex),
        Instruction::inh("dey", Instruction::op_dey),
        // TODO: Maybe we should have a single transfer instruction as well, which can go to one of tax txa tay tya txs tsx etc.
        Instruction::inh("tax", Instruction::op_tax),
        Instruction::inh("tay", Instruction::op_tay),
        Instruction::inh("txa", Instruction::op_txa),
        Instruction::inh("tya", Instruction::op_tya),
        Instruction::inh("asl a", Instruction::op_asl),
        Instruction::inh("rol", Instruction::op_rol),
        Instruction::inh("ror", Instruction::op_ror),
        Instruction::inh("lsr", Instruction::op_lsr),
        Instruction::inh("clc", Instruction::op_clc),
        Instruction::inh("sec", Instruction::op_sec),
        Instruction::imm("adc", Instruction::op_adc_dp),
        Instruction::abs("adc", Instruction::op_adc_dp),
        Instruction::abs("lda", Instruction::op_lda),
        Instruction::abs("sta", Instruction::op_sta),
        Instruction::abs("stx", Instruction::op_stx),
        Instruction::abs("sty", Instruction::op_sty),
    ]
}

pub fn mos65c02() -> Vec<Instruction> {
    vec![
        Instruction::inh("ina", Instruction::op_ina),
        Instruction::inh("dea", Instruction::op_dea),
        Instruction::inh("stz", Instruction::op_stz),
    ]
    .into_iter()
    .chain(mos6502())
    .collect()
}

pub fn z80() -> Vec<Instruction> {
    Vec::new()
}

pub fn i8080() -> Vec<Instruction> {
    Vec::new()
}

pub fn i8085() -> Vec<Instruction> {
    Vec::new()
}

pub fn iz80() -> Vec<Instruction> {
    Vec::new()
}

pub fn pic12() -> Vec<Instruction> {
    // Not sure yet how we're going to deal with the PIC instructions that
    // either write the result to W or back to the memory.
    vec![Instruction::abs("clrf", Instruction::op_stz)]
}

pub fn pic14() -> Vec<Instruction> {
    pic12()
}

pub fn pic16() -> Vec<Instruction> {
    pic14()
}
