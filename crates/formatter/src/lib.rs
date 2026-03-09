/// Lightweight TypeScript formatter.
///
/// Intentionally minimal — generated code should be consistent,
/// not necessarily "pretty". Avoids the Prettier dependency that
/// slows down openapi-typescript by 3x.
pub fn format(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut prev_was_empty = false;

    for line in input.lines() {
        let is_empty = line.trim().is_empty();

        // Collapse multiple blank lines into one
        if is_empty && prev_was_empty {
            continue;
        }

        output.push_str(line);
        output.push('\n');
        prev_was_empty = is_empty;
    }

    output
}
