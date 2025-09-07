use heapless::String;

/// Maximum NMEA sentence length is 82 chars (including `$` and `\r\n`)
const MAX_LEN: usize = 82;

pub struct NmeaAssembler {
    buf: String<MAX_LEN>,
}

impl NmeaAssembler {
    pub fn new() -> Self {
        Self { buf: String::new() }
    }

    /// Push one byte into the state machine.
    /// Returns `Some(sentence)` when a full `$...<CR><LF>` is complete.
    pub fn push_byte(&mut self, byte: u8) -> Option<String<MAX_LEN>> {
        match byte {
            b'$' => {
                // Start of new sentence
                self.buf.clear();
                let _ = self.buf.push('$');
                None
            }
            b'\r' => {
                // Ignore, wait for \n
                None
            }
            b'\n' => {
                if !self.buf.is_empty() {
                    // End of sentence
                    let mut out = String::new();
                    core::mem::swap(&mut out, &mut self.buf);
                    return Some(out);
                }
                None
            }
            _ => {
                if !self.buf.is_empty() {
                    // Inside a sentence, push byte if capacity allows
                    let _ = self.buf.push(byte as char);
                }
                None
            }
        }
    }
}
