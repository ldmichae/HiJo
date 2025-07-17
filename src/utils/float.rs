pub struct FloatToString {
    buffer: [u8; 32], // Sufficiently large buffer for most f64 representations
    len: usize,
    precision: u8,
}
impl FloatToString {
    pub fn new(precision: u8) -> Self {
        FloatToString {
            buffer: [0; 32],
            len: 0,
            precision: precision,

        }
    }

    pub fn convert(&mut self, value: f64) -> &str {
        self.len = 0; // Reset length for a new conversion

        // Handle special cases: NaN, Infinity
        if value.is_nan() {
            let _ = self.write_str("NaN");
        } else if value.is_infinite() {
            if value.is_sign_positive() {
                let _ = self.write_str("inf");
            } else {
                let _ = self.write_str("-inf");
            }
        } else {
            // Determine the sign
            let mut num = value;
            let is_negative = num < 0.0;
            if is_negative {
                num = -num;
                let _ = self.write_char(b'-');
            }

            // Extract integer part
            let integer_part = num as u64;
            let mut fractional_part = num - integer_part as f64;

            // Convert integer part to string
            if integer_part == 0 {
                let _ = self.write_char(b'0');
            } else {
                let mut temp_integer = integer_part;
                let mut temp_buffer = [0u8; 20]; // Buffer for integer part, max 20 digits for u64
                let mut i = temp_buffer.len();
                while temp_integer > 0 {
                    i -= 1;
                    temp_buffer[i] = b'0' + (temp_integer % 10) as u8;
                    temp_integer /= 10;
                }
                let _ =
                    self.write_str(unsafe { core::str::from_utf8_unchecked(&temp_buffer[i..]) });
            }

            // Add decimal point if there's a fractional part or if we want fixed precision
            if fractional_part > 0.0 && self.precision > 0 {
                let _ = self.write_char(b'.');

                // Convert fractional part to string
                for _ in 0..self.precision {
                    fractional_part *= 10.0;
                    let digit = fractional_part as u8;
                    let _ = self.write_char(b'0' + digit);
                    fractional_part -= digit as f64;
                }
            }
        }

        // Safety: We ensure that `self.buffer[0..self.len]` always contains valid UTF-8.
        unsafe { core::str::from_utf8_unchecked(&self.buffer[0..self.len]) }
    }

    fn write_char(&mut self, c: u8) -> core::fmt::Result {
        if self.len < self.buffer.len() {
            self.buffer[self.len] = c;
            self.len += 1;
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }

    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &byte in s.as_bytes() {
            self.write_char(byte)?;
        }
        Ok(())
    }
}
