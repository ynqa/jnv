pub fn add_to_nearest_integer(renderer: &mut promkit::text_editor::Renderer, value: isize) -> bool {
    let cursor = renderer.texteditor.position();
    let chars = renderer.texteditor.text_without_cursor().chars();
    let mut previous = None;
    let mut previous_distance = usize::MAX;
    let mut pos = 0;
    while pos < chars.len() {
        match find_number(&chars, pos) {
            Some((start, end)) => {
                let distance = distance_from(start, end, cursor);
                if distance < previous_distance {
                    previous = Some((start, end));
                    previous_distance = distance;
                }
                if distance == 0 {
                    pos = chars.len()
                } else {
                    pos = end + 1;
                }
            }
            None => pos = chars.len(),
        }
    }
    match previous {
        None => false,
        Some((start, end)) => {
            let before: String = chars[0..start].into_iter().collect();
            let after: String = chars[end..].into_iter().collect();
            let number: String = chars[start..end].into_iter().collect();
            let current_value: isize = match number.parse() {
                Ok(value) => value,
                Err(_) => return false,
            };
            let new_value = current_value + value;
            let new_query = format!("{before}{new_value}{after}");
            renderer.texteditor.replace(&new_query);
            let mut npos = renderer.texteditor.position();
            while npos > cursor {
                renderer.texteditor.backward();
                npos = renderer.texteditor.position();
            }
            true
        }
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

fn find_number(chars: &Vec<char>, start: usize) -> Option<(usize, usize)> {
    if start >= chars.len() {
        return None;
    }
    match chars[start..]
        .iter()
        .position(|ch| *ch == '-' || ch.is_digit(10))
    {
        Some(num_start) => {
            let ch = chars[num_start];
            eprintln!("matched: {ch} at offset {num_start}");
            let end = chars[start + num_start + 1..]
                .iter()
                .position(|ch| !ch.is_digit(10));
            match end {
                Some(end) => Some((start + num_start, start + num_start + 1 + end)),
                None => Some((start + num_start, chars.len())),
            }
        }
        None => None,
    }
}
