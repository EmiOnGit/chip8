pub fn map_op(instr: u16) -> String {
    let b0 = (instr & 0xFF00) >> 8 as u8; // To get first byte, & the 8 leftmost bits which removes the 8 rightmost, then shift by 8 to the right to make the u8 conversion contain the bits originally on the left.
                                          // println!("instr: {instr:x}, pc: {pc:x}", pc = self.pc);
    let b1 = (instr & 0x00FF) as u8; // To get the second byte, just & the 8 rightmost bits, which removes the leftmost bits. The remaining bits are already at the rightmost position so no need to shift before converting to u8.

    let op = (b0 & 0xF0) >> 4 as u8; // first nibble, the instruction. Keep 4 leftmost bits, then shift them to the right-hand side.
    let x = (b0 & 0x0F) as usize; // second nibble, register lookup! Only keep rightmost bits.
    let y = ((b1 & 0xF0) >> 4) as usize; // third nibble, register lookup! Keep leftmost bits, shift 4 to left.
    let n = b1 & 0x0F; // fourth nibble, 4 bit number
    let nn = b1; // NN = second byte
    let nnn = (instr & 0x0FFF) as u16; // NNN = second, third and fourth nibbles, obtained by ANDing by b00001111 11111111 masking away the first nibble.
    match (op, x, y, n) {
        (0x0, 0x0, 0xe, 0x0) => "clear".into(),

        (0x0, 0x0, 0xe, 0xe) => "return from subroutine".into(),

        (0x1, _, _, _) => format!("jmp to {nnn:x}"),
        (0x2, _, _, _) => format!("push subroutine {nnn:x}"),
        (0x3, _, _, _) => format!("skip if r[{x}] == {nn:x}"),
        (0x4, _, _, _) => format!("skip if r[{x}] != {nn:x}"),
        (0x5, _, _, 0) => format!("skip if r[{x}] == r[{y}]"),
        (0x6, _, _, _) => format!("r[{x}] = {nn:x}"),
        (0x7, _, _, _) => format!("r[{x}] += {nn:x}"),
        (0x8, _, _, 0) => format!("r[{x}] = r[{y}]"),
        (0x8, _, _, 1) => format!("r[{x}] = r[{x}] | r[{y}]"),
        (0x8, _, _, 2) => format!("r[{x}] = r[{x}] & r[{y}]"),
        (0x8, _, _, 3) => format!("r[{x}] = r[{x}] ^ r[{y}]"),
        (0x8, _, _, 4) => format!("r[{x}] = r[{x}] + r[{y}]"),
        (0x8, _, _, 5) => format!("r[{x}] = r[{x}] - r[{y}]"),
        (0x8, _, _, 6) => format!("r[{x}] = r[{x}] >> 1"),
        (0x8, _, _, 7) => format!("r[{x}] = r[{y}] - r[{x}]"),
        (0x8, _, _, 0xe) => format!("r[{x}] = r[{x}] << 1"),
        (0x9, _, _, 0) => format!("skip if r[{x}] != r[{y}]"),
        (0xa, _, _, _) => format!("i = {nnn}"),
        (0xb, _, _, _) => format!("pc = r[{x}] + {nnn:x}"),
        (0xc, _, _, _) => format!("rand & {nn:x}"),
        (0xd, _, _, _) => format!("draw at x=r[{x}],y=r[{y}]"),
        (0xe, _, 9, 0xe) => format!("skip if r[{x}]) pressed"),
        (0xe, _, 0xa, 1) => format!("skip if r[{x}]) not pressed"),
        (0xf, _, 0, 7) => format!("r[{x}] = delay"),
        (0xf, _, 1, 5) => format!("delay = r[{x}]"),
        (0xf, _, 1, 8) => format!("sound = r[{x}]"),
        (0xf, _, 1, 0xe) => format!("i += r[{x}]"),
        (0xf, _, 0, 0xa) => "wait for any keypres".into(),

        (0xf, _, 2, 9) => format!("i = r[{x}]th CHAR"),
        (0xf, _, 5, 5) => "store regs in mem".into(),
        (0xf, _, 6, 5) => "load regs from mem".into(),
        _ => "".into(),
    }
}
