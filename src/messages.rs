use std::fmt;

pub const TELEGRAM_MAX_MSG_LEN: usize = 4096;

pub fn format_large_msg<T, F>(head: String, data: &[T], line_format_fn: F) -> Vec<String>
where
    F: Fn(&T) -> String,
{
    let mut msgs = vec![head];
    for item in data {
        let line = line_format_fn(item);
        if msgs.last_mut().unwrap().len() + line.len() > TELEGRAM_MAX_MSG_LEN {
            msgs.push(line);
        } else {
            let msg = msgs.last_mut().unwrap();
            msg.push('\n');
            msg.push_str(&line);
        }
    }
    msgs
}

pub struct Escape<'a>(pub &'a str);

impl<'a> fmt::Display for Escape<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        // https://core.telegram.org/bots/api#html-style
        let Escape(s) = *self;
        let pile_o_bits = s;
        let mut last = 0;
        for (i, ch) in s.bytes().enumerate() {
            match ch as char {
                '<' | '>' | '&' | '"' => {
                    fmt.write_str(&pile_o_bits[last..i])?;
                    let s = match ch as char {
                        '>' => "&gt;",
                        '<' => "&lt;",
                        '&' => "&amp;",
                        '"' => "&quot;",
                        _ => unreachable!(),
                    };
                    fmt.write_str(s)?;
                    last = i + 1;
                }
                _ => {}
            }
        }

        if last < s.len() {
            fmt.write_str(&pile_o_bits[last..])?;
        }
        Ok(())
    }
}
