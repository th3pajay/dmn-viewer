use dmn_viewer::report::{FailOn, Report};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: dmn-check <file.dmn | dir | glob> [...] [--test suite.json] [--coverage]
                 [--json | --junit | --sarif] [--fail-on error|warn|never]
       <name>.tests.json next to <name>.dmn is run automatically";

fn usage(problem: &str) -> ExitCode {
    if !problem.is_empty() {
        eprintln!("dmn-check: {problem}");
    }
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut inputs = Vec::new();
    let mut tests: Option<PathBuf> = None;
    let mut coverage = false;
    let mut format: Option<String> = None;
    let mut fail_on = FailOn::Error;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--test" | "-t" => tests = args.next().map(PathBuf::from),
            "--coverage" => coverage = true,
            "--json" | "--junit" | "--sarif" => {
                if format.as_ref().is_some_and(|f| *f != a) {
                    return usage("pick one of --json, --junit and --sarif");
                }
                format = Some(a);
            }
            "--fail-on" => match args.next().as_deref().and_then(FailOn::parse) {
                Some(f) => fail_on = f,
                None => return usage("--fail-on takes error, warn or never"),
            },
            _ => inputs.push(a),
        }
    }
    if inputs.is_empty() {
        return usage("");
    }
    let report = Report::check(&inputs, tests.as_deref());
    let out = &mut io::stdout().lock();
    // A closed pipe (e.g. `| head`) is not a check failure; the exit code still reports the result.
    let _ = match format.as_deref() {
        Some("--json") => writeln!(out, "{}", report.json(fail_on)),
        Some("--junit") => write!(out, "{}", report.junit(fail_on)),
        Some("--sarif") => writeln!(out, "{}", report.sarif()),
        _ => report.write_text(coverage, out, &mut io::stderr().lock()),
    };
    if report.failed(fail_on) { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
