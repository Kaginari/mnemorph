//! Assembling a body's report: its own lines, and the tool output it quotes.

/// Quotes a tool's output into `report`, one line per output line.
pub fn quote_output(report: &mut String, output: &str) {
    for line in output.lines() {
        report.push_str(line);
        report.push('\n');
    }
}

/// Closes the report with its byte count.
pub fn close(report: &mut String) {
    let n = report.len();
    report.push_str(&format!("@E {n}\n"));
}
