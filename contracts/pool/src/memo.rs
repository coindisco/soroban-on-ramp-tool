const SYMBOLS: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

fn get_next_symbol(sym: u8) -> (u8, bool) {
    let sym_bytes = SYMBOLS.as_bytes();
    let sym_idx = sym_bytes.iter().position(|&r| r == sym).unwrap();
    if sym_idx == sym_bytes.len() - 1 {
        (sym_bytes[0], true)
    } else {
        (sym_bytes[sym_idx + 1], false)
    }
}

pub(crate) fn generate_next_memo(memo_original: &[u8; 28]) -> [u8; 28] {
    let mut memo_bytes = [0u8; 28];
    for i in 0..28 {
        memo_bytes[i] = memo_original[i];
    }
    for i in (0..memo_bytes.len()).rev() {
        let (next, overflow) = get_next_symbol(memo_bytes[i]);
        memo_bytes[i] = next;
        if !overflow {
            break;
        }
    }
    memo_bytes
}
