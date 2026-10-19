//! The CSV dialect of Python's `csv` module as `reference/python/core/lexo/util.py` uses it: the reader (`read_rows`, with the same state machine as `_csv.c`, so odd files give the
//! same rows) and the writer (`csv_text`: quotes only what needs it, `\r\n` line ends).
use crate::util::py_splitlines;

/// What ends a field at a line end: the state, the fields, the field and its length are passed in.
type Finish<'a> = dyn Fn(&mut State, &mut Vec<String>, &mut String, &mut usize) + 'a;

#[derive(Clone, Copy, PartialEq)]
enum State {
    StartRecord,
    StartField,
    InField,
    InQuotedField,
    QuoteInQuotedField,
    EatCrnl,
}

/// Lines of a text like `io.StringIO(text, newline="")` yields them: the end of a line is `\n`, `\r` or `\r\n` and stays in the line.
fn lines_keeping_ends(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < b.len() {
        if b[i] == b'\n' {
            out.push(&text[start..=i]);
            start = i + 1;
        } else if b[i] == b'\r' {
            let end = if b.get(i + 1) == Some(&b'\n') { i + 1 } else { i };
            out.push(&text[start..=end]);
            i = end;
            start = end + 1;
        }
        i += 1;
    }
    if start < b.len() {
        out.push(&text[start..]);
    }
    out
}

/// `csv.reader(io.StringIO(text, newline=""), delimiter=delimiter)`: every record, empty lines as empty records.
pub fn parse(text: &str, delimiter: char) -> Vec<Vec<String>> {
    const EOL: char = '\0';
    let quote = '"';
    let mut records: Vec<Vec<String>> = Vec::new();
    let (mut fields, mut field): (Vec<String>, String) = (Vec::new(), String::new());
    let mut state = State::StartRecord;
    let mut field_len = 0usize;
    for line in lines_keeping_ends(text) {
        for c in line.chars().chain(std::iter::once(EOL)) {
            let end_of_line = c == '\n' || c == '\r' || c == EOL;
            let finish = |state: &mut State, fields: &mut Vec<String>, field: &mut String, field_len: &mut usize| {
                fields.push(std::mem::take(field));
                *field_len = 0;
                *state = if c == EOL { State::StartRecord } else { State::EatCrnl };
            };
            match state {
                State::StartRecord => {
                    if c == EOL {
                        continue;
                    }
                    if c == '\n' || c == '\r' {
                        state = State::EatCrnl;
                        continue;
                    }
                    state = State::StartField;
                    start_field(c, delimiter, quote, &mut state, &mut fields, &mut field, &mut field_len, end_of_line, &finish);
                }
                State::StartField => start_field(c, delimiter, quote, &mut state, &mut fields, &mut field, &mut field_len, end_of_line, &finish),
                State::InField => {
                    if end_of_line {
                        finish(&mut state, &mut fields, &mut field, &mut field_len);
                    } else if c == delimiter {
                        fields.push(std::mem::take(&mut field));
                        field_len = 0;
                        state = State::StartField;
                    } else {
                        field.push(c);
                        field_len += 1;
                    }
                }
                State::InQuotedField => {
                    if c == EOL {
                    } else if c == quote {
                        state = State::QuoteInQuotedField;
                    } else {
                        field.push(c);
                        field_len += 1;
                    }
                }
                State::QuoteInQuotedField => {
                    if c == quote {
                        field.push(c);
                        field_len += 1;
                        state = State::InQuotedField;
                    } else if c == delimiter {
                        fields.push(std::mem::take(&mut field));
                        field_len = 0;
                        state = State::StartField;
                    } else if end_of_line {
                        finish(&mut state, &mut fields, &mut field, &mut field_len);
                    } else {
                        field.push(c);
                        field_len += 1;
                        state = State::InField;
                    }
                }
                State::EatCrnl => {
                    if c == EOL {
                        state = State::StartRecord;
                    }
                }
            }
        }
        if state == State::StartRecord {
            records.push(std::mem::take(&mut fields));
        }
    }
    // end of input inside a record: a field that has begun, or an open quote, is kept
    if state != State::StartRecord && (field_len != 0 || state == State::InQuotedField) {
        fields.push(field);
        records.push(fields);
    }
    records
}

#[allow(clippy::too_many_arguments)]
fn start_field(
    c: char,
    delimiter: char,
    quote: char,
    state: &mut State,
    fields: &mut Vec<String>,
    field: &mut String,
    field_len: &mut usize,
    end_of_line: bool,
    finish: &Finish<'_>,
) {
    if end_of_line {
        finish(state, fields, field, field_len);
    } else if c == quote {
        *state = State::InQuotedField;
    } else if c == delimiter {
        fields.push(std::mem::take(field));
        *field_len = 0;
    } else {
        field.push(c);
        *field_len += 1;
        *state = State::InField;
    }
}

/// The rows of a CSV text (`,` or `;` as the delimiter, chosen by the first line), empty records dropped. A BOM at the start is removed first.
pub fn read_rows_text(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = if crate::util::py_strip(text).is_empty() { String::new() } else { py_splitlines(text).into_iter().next().unwrap_or_default() };
    let delim = if first.matches(';').count() > first.matches(',').count() { ';' } else { ',' };
    parse(text, delim).into_iter().filter(|r| !r.is_empty()).collect()
}

/// `csv.writer(..., delimiter=delimiter).writerows(rows)` with the default dialect: a field is quoted when it holds the delimiter, a quote, `\r` or `\n`
/// (quotes are doubled); a row that is one empty field is written as `""`; lines end with `\r\n`.
pub fn csv_text(rows: &[Vec<String>], delimiter: char) -> String {
    let mut out = String::new();
    for row in rows {
        let mut line = String::new();
        for (i, f) in row.iter().enumerate() {
            if i > 0 {
                line.push(delimiter);
            }
            if f.chars().any(|c| c == delimiter || c == '"' || c == '\r' || c == '\n') {
                line.push('"');
                line.push_str(&f.replace('"', "\"\""));
                line.push('"');
            } else {
                line.push_str(f);
            }
        }
        if row.len() == 1 && row[0].is_empty() {
            line = "\"\"".into();
        }
        out.push_str(&line);
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(t: &str) -> Vec<Vec<String>> {
        read_rows_text(t)
    }

    #[test]
    fn quoted_fields_may_hold_the_delimiter_quotes_and_line_breaks() {
        assert_eq!(rows("a;b\r\n\"x;y\";\"q\"\"uote\"\r\n\"multi\nline\";z\r\n"), vec![vec!["a", "b"], vec!["x;y", "q\"uote"], vec!["multi\nline", "z"]]);
    }

    #[test]
    fn odd_files_read_like_python() {
        assert_eq!(rows("a;\"b\nc;d\n"), vec![vec!["a".to_string(), "b\nc;d\n".to_string()]]);
        assert_eq!(rows("a;b\"c;\"d\"e\n"), vec![vec!["a", "b\"c", "de"]]);
        assert_eq!(rows("a;b\r1;2\r"), vec![vec!["a", "b"], vec!["1", "2"]]);
        assert_eq!(rows("  \n\n"), vec![vec!["  "]]);
        assert!(rows("").is_empty());
    }

    #[test]
    fn the_writer_quotes_only_what_needs_it() {
        let r = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(csv_text(&[r(&["a", " lead", "b;c", "q\"x"]), r(&[""]), vec![], r(&["", ""])], ';'), "a; lead;\"b;c\";\"q\"\"x\"\r\n\"\"\r\n\r\n;\r\n");
    }
}
