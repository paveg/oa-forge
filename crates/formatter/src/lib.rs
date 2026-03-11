/// Lightweight TypeScript formatter.
///
/// Intentionally minimal — generated code should be consistent,
/// not necessarily "pretty". Avoids the Prettier dependency that
/// slows down openapi-typescript by 3x.
pub fn format(input: &str) -> String {
    let mut lines: Vec<&str> = input.lines().collect();

    // Sort import blocks: group `import type` before `import`, then alphabetically
    sort_imports(&mut lines);

    let mut output = String::with_capacity(input.len());
    let mut prev_was_empty = false;

    for line in &lines {
        let is_empty = line.trim().is_empty();

        // Collapse multiple blank lines into one
        if is_empty && prev_was_empty {
            continue;
        }

        // Ensure consistent 2-space indentation (normalize tabs to 2 spaces)
        if line.starts_with('\t') {
            let indent_count = line.chars().take_while(|&c| c == '\t').count();
            let spaces = "  ".repeat(indent_count);
            output.push_str(&spaces);
            output.push_str(line.trim_start_matches('\t'));
        } else {
            output.push_str(line);
        }

        output.push('\n');
        prev_was_empty = is_empty;
    }

    output
}

/// Sort import statements at the top of the file.
/// Groups: `import type` first, then `import`, sorted alphabetically within each group.
fn sort_imports(lines: &mut [&str]) {
    // Find the contiguous import block at the top (after header comment)
    let import_start = lines
        .iter()
        .position(|l| l.starts_with("import ") || l.starts_with("import type "));
    let Some(start) = import_start else {
        return;
    };

    let end = lines[start..]
        .iter()
        .position(|l| {
            !l.starts_with("import ") && !l.starts_with("import type ") && !l.trim().is_empty()
        })
        .map(|pos| start + pos)
        .unwrap_or(lines.len());

    // Collect non-empty import lines
    let mut type_imports: Vec<&str> = Vec::new();
    let mut value_imports: Vec<&str> = Vec::new();

    for line in &lines[start..end] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("import type ") {
            type_imports.push(trimmed);
        } else if trimmed.starts_with("import ") {
            value_imports.push(trimmed);
        }
    }

    type_imports.sort();
    value_imports.sort();

    // Rebuild the import section
    let mut sorted: Vec<&str> = Vec::new();
    sorted.extend_from_slice(&type_imports);
    sorted.extend_from_slice(&value_imports);

    // Replace the import block in-place
    let import_count = sorted.len();

    // Copy sorted imports into the original slots, filling excess with empty lines
    for (i, slot) in lines[start..end].iter_mut().enumerate() {
        if i < import_count {
            *slot = sorted[i];
        } else {
            *slot = "";
        }
    }

    // Extra "" entries will be collapsed by the blank-line logic in format()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_blank_lines() {
        let input = "a\n\n\nb\n";
        let result = format(input);
        assert_eq!(result, "a\n\nb\n");
    }

    #[test]
    fn normalizes_tabs_to_spaces() {
        let input = "\tname: string;\n\t\tnested: boolean;\n";
        let result = format(input);
        assert_eq!(result, "  name: string;\n    nested: boolean;\n");
    }

    #[test]
    fn sorts_imports() {
        let input = "// Header\n\nimport { z } from './z';\nimport type { B } from './b';\nimport type { A } from './a';\nimport { x } from './x';\n\nexport interface Foo {}\n";
        let result = format(input);
        assert!(result.contains("import type { A }"));
        // type imports should come before value imports
        let type_a_pos = result.find("import type { A }").unwrap();
        let import_x_pos = result.find("import { x }").unwrap();
        assert!(type_a_pos < import_x_pos, "type imports should come first");
    }
}
