use gag::Gag;
use promkit::serde_json::Value;

pub fn add_to_nearest_array_index(renderer: &mut crate::jnv::render::Renderer, value: isize) -> bool {
    let query_editor_after_mut = renderer.query_editor_snapshot.after_mut();
    let cursor = query_editor_after_mut.texteditor.position();
    let query = query_editor_after_mut
        .texteditor
        .text_without_cursor()
        .to_string();
    let mut previous = None;
    let mut previous_distance = usize::MAX;
    let mut pos = 0;

    while pos < query.len() {
        match find_array_index(&query, pos) {
            Some((start, end)) => {
                let distance = distance_from(start, end, cursor);
                if distance < previous_distance {
                    previous = Some((start, end));
                    previous_distance = distance;
                }
                if distance == 0 {
                    pos = query.len()
                } else {
                    pos = end + 1;
                }
            }
            None => {
                pos += 1
            }
        }
    }
    match previous {
        None => false,
        Some((start, end)) => {
            let before = &query[0..start + 1]; // ends with '['
            let after = &query[end..]; // starts with ']'
            let number = &query[start + 1..end];
            let current_value: isize = match number.parse() {
                Ok(value) => value,
                Err(_) => return false,
            };
            let mut new_value = current_value + value;

            // array bound checking and cycling
            let array = &before[..before.len() - 1];
            let len = query_array_length(&renderer.input_json_stream, array);
            if len == -1 {
                return false;
            }

            if current_value == 0 && new_value == -1 {
                new_value = len - 1;
            }
            if new_value > 0 && new_value >= len {
                new_value = 0;
            } else if new_value < -len {
                new_value = -1;
            }
            let new_query = format!("{before}{new_value}{after}");
            query_editor_after_mut.texteditor.replace(&new_query);
            let mut npos = query_editor_after_mut.texteditor.position();
            while npos > cursor {
                query_editor_after_mut.texteditor.backward();
                npos = query_editor_after_mut.texteditor.position();
            }
            true
        }
    }
}

fn query_array_length(input_json_stream: &[Value], array: &str) -> isize {
    let stream_len = input_json_stream.len();
    if stream_len == 0 || stream_len > 1 {
        // honestly, don't now how to handle it
        return -1;
    }
    let query_len = format!("{array} | length");
    let v = &input_json_stream[0];

    let _ignore_err = Gag::stderr().unwrap();

    match j9::run(&query_len, &v.to_string()) {
        Ok(ret) => {
            if ret.is_empty() {
                -1
            } else {
                ret[0].parse().unwrap()
            }
        }
        Err(_e) => -1,
    }
}

fn distance_from(start: usize, end: usize, cursor: usize) -> usize {
    if start <= cursor && cursor < end {
        0
    } else if cursor < start {
        start - cursor
    } else {
        cursor - end
    }
}
fn find_array_index(query: &str, start: usize) -> Option<(usize, usize)> {
    let iter = query.chars().enumerate().skip(start);
    let mut iter = iter.skip_while(|(_, ch)| *ch != '[');
    let start = match iter.next() {
        None => return None,
        Some((pos, _)) => pos,
    };
    let mut iter = iter.skip_while(|(i, ch)| *i == start + 1 && *ch == '-');
    let _minus = iter.next().is_some();
    let mut iter = iter.skip_while(|(_, ch)| ch.is_ascii_digit());
    match iter.next() {
        Some((end, ']')) => Some((start, end)),
        _ => None
    }
}
