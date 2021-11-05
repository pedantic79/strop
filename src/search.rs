use crate::machine::Instruction;
use crate::State;

pub struct Schema<'a> {
    live_in: Vec<Box<dyn for<'r> Fn(&'r mut State, i8)>>,
    live_out: Vec<Box<dyn for<'r> Fn(&'r State) -> Option<i8> + 'a>>,
}

impl<'a> Schema<'_> {
    pub fn new(
        live_in: Vec<Box<dyn for<'r> Fn(&'r mut State, i8)>>,
        live_out: Vec<Box<dyn for<'r> Fn(&'r State) -> Option<i8> + 'a>>,
    ) -> Schema {
        Schema { live_in, live_out }
    }
}

fn run_program(prog: &[Instruction], schema: &Schema, inputs: &[i8]) -> Option<State> {
    let mut s = State::new();

    for (func, val) in schema.live_in.iter().zip(inputs) {
        (func)(&mut s, *val);
    }
    if prog.iter().all(|i| (i.operation)(i, &mut s)) {
        Some(s)
    } else {
        None
    }
}

pub fn equivalence(
    prog: &[Instruction],
    schema: &Schema,
    test_cases: &[(Vec<i8>, Vec<i8>)],
) -> bool {
    for tc in test_cases {
        if let Some(state) = run_program(prog, schema, &tc.0) {
            for (func, val) in schema.live_out.iter().zip(&tc.1) {
                let result = func(&state);
                if result != Some(*val) {
                    return false;
                }
            }
        } else {
            return false;
        }
    }
    true
}

pub fn exhaustive_search(
    found_it: &dyn Fn(&[Instruction]) -> bool,
    instructions: Vec<Instruction>,
    constants: Vec<i8>,
    vars: Vec<u16>,
) {
    let instrs = {
        let mut temp = Vec::new();
        for ins in &instructions {
            ins.vectorize(&mut temp, &constants, &vars);
        }
        temp
    };

    fn try_all(
        term: &dyn Fn(&[Instruction]) -> bool,
        prog: &mut Vec<Instruction>,
        instrs: &[Instruction],
        len: u32,
    ) -> bool {
        if len == 0 {
            term(prog)
        } else {
            for ins in instrs {
                prog.push(*ins);
                if try_all(term, prog, instrs, len - 1) {
                    return true;
                }
                prog.pop();
            }
            false
        }
    }

    let t: &dyn Fn(&[Instruction]) -> bool = &|v| -> bool { found_it(v) };

    for i in 1..10 {
        println!("Trying programs of length {}.", i);
        if try_all(&t, &mut Vec::new(), &instrs, i) {
            return;
        }
    }
}
