use dmn_viewer::lint::{Issue, Severity, Tally, tally};

fn issue(severity: Severity, decision: Option<usize>) -> Issue {
    Issue { severity, code: "t", decision, rules: Vec::new(), column: None, message: String::new() }
}

#[test]
fn one_pass_counts_issues_and_errors_per_decision_and_for_the_file() {
    let issues = [
        issue(Severity::Error, Some(0)),
        issue(Severity::Warning, Some(0)),
        issue(Severity::Error, Some(2)),
        issue(Severity::Info, None),
        issue(Severity::Error, None),
        issue(Severity::Error, Some(9)),
    ];
    let (per, global) = tally(&issues, 3);
    assert_eq!(per, vec![Tally { issues: 2, errors: 1 }, Tally::default(), Tally { issues: 1, errors: 1 }]);
    assert_eq!(global, Tally { issues: 2, errors: 1 });
    assert_eq!(per[0].with(global), Tally { issues: 4, errors: 2 });
}

#[test]
fn tally_matches_filtering_each_decision() {
    let issues: Vec<Issue> = (0..500).map(|i| issue(if i % 3 == 0 { Severity::Error } else { Severity::Warning }, (i % 7 != 0).then_some(i % 11))).collect();
    let (per, global) = tally(&issues, 11);
    for (d, t) in per.iter().enumerate() {
        let naive = issues.iter().filter(|x| x.decision == Some(d)).count();
        let errs = issues.iter().filter(|x| x.decision == Some(d) && x.severity == Severity::Error).count();
        assert_eq!((t.issues, t.errors), (naive, errs));
    }
    assert_eq!(global.issues, issues.iter().filter(|x| x.decision.is_none()).count());
}
