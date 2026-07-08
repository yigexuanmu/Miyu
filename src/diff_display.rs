use crate::config::DiffDisplayPluginConfig;
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use std::io::{self, Write};

#[derive(Clone)]
pub struct DiffLine {
    pub old_line_num: Option<usize>,
    pub new_line_num: Option<usize>,
    pub content: String,
    pub line_type: DiffLineType,
}

#[derive(PartialEq, Clone)]
pub enum DiffLineType {
    Context,
    Addition,
    Deletion,
    HunkHeader,
}

pub struct FileDiff {
    pub path: String,
    pub lines: Vec<DiffLine>,
}

pub fn compute_file_diff(old_content: &str, new_content: &str, path: &str) -> FileDiff {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();
    let mut diff_lines = Vec::new();

    let lcs = longest_common_subsequence(&old_lines, &new_lines);
    let mut old_idx = 0;
    let mut new_idx = 0;
    let mut lcs_idx = 0;

    while old_idx < old_lines.len() || new_idx < new_lines.len() {
        if lcs_idx < lcs.len()
            && old_idx < old_lines.len()
            && new_idx < new_lines.len()
            && old_lines[old_idx] == lcs[lcs_idx]
            && new_lines[new_idx] == lcs[lcs_idx]
        {
            diff_lines.push(DiffLine {
                old_line_num: Some(old_idx + 1),
                new_line_num: Some(new_idx + 1),
                content: old_lines[old_idx].to_string(),
                line_type: DiffLineType::Context,
            });
            old_idx += 1;
            new_idx += 1;
            lcs_idx += 1;
        } else if old_idx < old_lines.len()
            && (lcs_idx >= lcs.len() || old_lines[old_idx] != lcs[lcs_idx])
        {
            diff_lines.push(DiffLine {
                old_line_num: Some(old_idx + 1),
                new_line_num: None,
                content: old_lines[old_idx].to_string(),
                line_type: DiffLineType::Deletion,
            });
            old_idx += 1;
        } else if new_idx < new_lines.len() {
            diff_lines.push(DiffLine {
                old_line_num: None,
                new_line_num: Some(new_idx + 1),
                content: new_lines[new_idx].to_string(),
                line_type: DiffLineType::Addition,
            });
            new_idx += 1;
        }
    }

    FileDiff {
        path: path.to_string(),
        lines: diff_lines,
    }
}

fn longest_common_subsequence<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<&'a str> {
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    let mut result = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            result.push(a[i - 1]);
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] > dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    result.reverse();
    result
}

pub fn print_diff(diff: &FileDiff, config: &DiffDisplayPluginConfig) -> io::Result<()> {
    let mut stdout = io::stdout();
    let has_changes = diff.lines.iter().any(|l| l.line_type != DiffLineType::Context);

    if !has_changes {
        return Ok(());
    }

    let max_lines = config.max_lines;
    let context_lines = config.context_lines;
    let filtered_lines = filter_context_lines(&diff.lines, context_lines, max_lines);

    let width = terminal_width().unwrap_or(80);
    let box_width = width.min(120);

    if config.show_file_header {
        writeln!(stdout)?;
        draw_top_border(&mut stdout, &diff.path, box_width)?;
    }

    for line in filtered_lines.iter() {
        match line.line_type {
            DiffLineType::HunkHeader => {
                draw_hunk_line(&mut stdout, line, box_width)?;
            }
            DiffLineType::Context => {
                draw_context_line(&mut stdout, line, box_width)?;
            }
            DiffLineType::Addition => {
                draw_addition_line(&mut stdout, line, box_width)?;
            }
            DiffLineType::Deletion => {
                draw_deletion_line(&mut stdout, line, box_width)?;
            }
        }
    }

    if config.show_file_header {
        draw_bottom_border(&mut stdout, box_width)?;
    }

    stdout.flush()?;
    Ok(())
}

fn filter_context_lines(lines: &[DiffLine], context: usize, max_lines: usize) -> Vec<DiffLine> {
    let change_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.line_type != DiffLineType::Context)
        .map(|(i, _)| i)
        .collect();

    if change_indices.is_empty() {
        return lines.iter().take(max_lines).cloned().collect();
    }

    let mut included = vec![false; lines.len()];

    for &idx in &change_indices {
        let start = idx.saturating_sub(context);
        let end = (idx + context + 1).min(lines.len());
        for i in start..end {
            included[i] = true;
        }
    }

    let mut result: Vec<DiffLine> = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| included[*i])
        .map(|(_, l)| l.clone())
        .take(max_lines)
        .collect();

    let total_included = lines.iter().filter(|l| {
        let idx = lines.iter().position(|x| std::ptr::eq(x as *const DiffLine, *l as *const DiffLine));
        idx.map_or(false, |i| included[i])
    }).count();
    
    if result.len() < total_included {
        result.push(DiffLine {
            old_line_num: None,
            new_line_num: None,
            content: "...".to_string(),
            line_type: DiffLineType::HunkHeader,
        });
    }

    result
}

fn terminal_width() -> Option<usize> {
    crossterm::terminal::size().ok().map(|(w, _)| w as usize)
}

fn draw_top_border(stdout: &mut io::Stdout, title: &str, width: usize) -> io::Result<()> {
    write!(stdout, "╭─ ")?;
    write!(stdout, "{}", title)?;
    let remaining = width.saturating_sub(title.len() + 4);
    let padding = "─".repeat(remaining);
    writeln!(stdout, " {}", padding)?;
    Ok(())
}

fn draw_bottom_border(stdout: &mut io::Stdout, width: usize) -> io::Result<()> {
    let line = "─".repeat(width);
    writeln!(stdout, "╰{}╯", line)?;
    Ok(())
}

fn draw_hunk_line(stdout: &mut io::Stdout, line: &DiffLine, width: usize) -> io::Result<()> {
    write!(stdout, "│ ")?;
    write!(
        stdout,
        "{}",
        SetForegroundColor(Color::Cyan)
    )?;
    write!(stdout, "@@ ")?;
    if let (Some(old), Some(new)) = (line.old_line_num, line.new_line_num) {
        write!(stdout, "-{},+{} ", old, new)?;
    }
    write!(stdout, "@@")?;
    write!(stdout, "{}", ResetColor)?;
    let content_len = line.content.len() + 20;
    if content_len < width {
        let remaining = width - content_len;
        write!(stdout, " {}", line.content)?;
        write!(stdout, "{}", " ".repeat(remaining.saturating_sub(1)))?;
    }
    writeln!(stdout, "│")?;
    Ok(())
}

fn draw_context_line(stdout: &mut io::Stdout, line: &DiffLine, _width: usize) -> io::Result<()> {
    write!(stdout, "│ ")?;
    if let Some(old) = line.old_line_num {
        write!(stdout, "{:>4} ", old)?;
    } else if let Some(new) = line.new_line_num {
        write!(stdout, "{:>4} ", new)?;
    } else {
        write!(stdout, "     ")?;
    }
    write!(stdout, "  ")?;
    writeln!(stdout, "{}", line.content)?;
    Ok(())
}

fn draw_addition_line(stdout: &mut io::Stdout, line: &DiffLine, _width: usize) -> io::Result<()> {
    write!(stdout, "│ ")?;
    if let Some(new) = line.new_line_num {
        write!(stdout, "{:>4} ", new)?;
    } else {
        write!(stdout, "     ")?;
    }
    write!(
        stdout,
        "{}",
        SetForegroundColor(Color::Green)
    )?;
    write!(stdout, "+ ")?;
    write!(stdout, "{}", line.content)?;
    writeln!(stdout, "{}", ResetColor)?;
    Ok(())
}

fn draw_deletion_line(stdout: &mut io::Stdout, line: &DiffLine, _width: usize) -> io::Result<()> {
    write!(stdout, "│ ")?;
    if let Some(old) = line.old_line_num {
        write!(stdout, "{:>4} ", old)?;
    } else {
        write!(stdout, "     ")?;
    }
    write!(
        stdout,
        "{}",
        SetForegroundColor(Color::Red)
    )?;
    write!(stdout, "- ")?;
    write!(stdout, "{}", line.content)?;
    writeln!(stdout, "{}", ResetColor)?;
    Ok(())
}

pub fn print_file_diff(old_content: &str, new_content: &str, path: &str, config: &DiffDisplayPluginConfig) -> io::Result<()> {
    let diff = compute_file_diff(old_content, new_content, path);
    print_diff(&diff, config)
}
