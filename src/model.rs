use crate::unary::UnaryTests;
use std::borrow::Cow;

#[derive(Clone, Debug, Default)]
pub struct Definitions {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub dmn_version: Option<&'static str>,
    pub exporter: String,
    pub exporter_version: String,
    pub expression_language: Option<String>,
    pub decisions: Vec<Decision>,
    pub input_data: Vec<NamedElement>,
    pub knowledge_sources: Vec<NamedElement>,
    pub bkms: Vec<NamedElement>,
    pub duplicate_ids: Vec<String>,
}

impl Definitions {
    pub fn table(&self, decision: usize) -> Option<&DecisionTable> {
        match &self.decisions.get(decision)?.logic {
            DecisionLogic::Table(t) => Some(t),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NamedElement {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct Requirement {
    pub kind: String,
    pub href: String,
}

#[derive(Clone, Debug)]
pub struct Decision {
    pub id: String,
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub requirements: Vec<Requirement>,
    pub logic: DecisionLogic,
}

#[derive(Clone, Debug)]
pub enum DecisionLogic {
    Table(DecisionTable),
    Literal { text: String, lang: ExpressionLanguage, variable: Option<String> },
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Aggregation {
    Sum,
    Count,
    Min,
    Max,
    Unknown(String),
}

impl Aggregation {
    pub const ALL: [Aggregation; 4] = [Aggregation::Sum, Aggregation::Count, Aggregation::Min, Aggregation::Max];

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "SUM" => Aggregation::Sum,
            "COUNT" => Aggregation::Count,
            "MIN" => Aggregation::Min,
            "MAX" => Aggregation::Max,
            _ => Aggregation::Unknown(s.trim().to_string()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Aggregation::Sum => "SUM",
            Aggregation::Count => "COUNT",
            Aggregation::Min => "MIN",
            Aggregation::Max => "MAX",
            Aggregation::Unknown(s) => s,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum HitPolicy {
    #[default]
    Unique,
    First,
    Priority,
    Any,
    Collect(Option<Aggregation>),
    RuleOrder,
    OutputOrder,
    Unknown(String),
}

impl HitPolicy {
    pub const ALL: [HitPolicy; 7] = [HitPolicy::Unique, HitPolicy::First, HitPolicy::Priority, HitPolicy::Any, HitPolicy::Collect(None), HitPolicy::RuleOrder, HitPolicy::OutputOrder];

    pub fn parse(hit_policy: Option<&str>, aggregation: Option<&str>) -> Self {
        let Some(h) = hit_policy.map(str::trim) else { return HitPolicy::Unique };
        match h.to_ascii_uppercase().as_str() {
            "UNIQUE" => HitPolicy::Unique,
            "FIRST" => HitPolicy::First,
            "PRIORITY" => HitPolicy::Priority,
            "ANY" => HitPolicy::Any,
            "COLLECT" => HitPolicy::Collect(aggregation.map(Aggregation::parse)),
            "RULE ORDER" => HitPolicy::RuleOrder,
            "OUTPUT ORDER" => HitPolicy::OutputOrder,
            _ => HitPolicy::Unknown(h.to_string()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            HitPolicy::Unique => "UNIQUE",
            HitPolicy::First => "FIRST",
            HitPolicy::Priority => "PRIORITY",
            HitPolicy::Any => "ANY",
            HitPolicy::Collect(_) => "COLLECT",
            HitPolicy::RuleOrder => "RULE ORDER",
            HitPolicy::OutputOrder => "OUTPUT ORDER",
            HitPolicy::Unknown(s) => s,
        }
    }

    pub fn aggregation(&self) -> Option<&Aggregation> {
        match self {
            HitPolicy::Collect(a) => a.as_ref(),
            _ => None,
        }
    }

    pub fn same_kind(&self, other: &HitPolicy) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    pub fn unsupported(&self) -> Option<String> {
        match self {
            HitPolicy::Unknown(s) => Some(format!("Unsupported hit policy \"{s}\"")),
            HitPolicy::Collect(Some(Aggregation::Unknown(s))) => Some(format!("Unsupported aggregation \"{s}\"")),
            _ => None,
        }
    }

    pub fn returns_rows(&self) -> bool {
        matches!(self, HitPolicy::Collect(None) | HitPolicy::RuleOrder | HitPolicy::OutputOrder)
    }
}

impl std::fmt::Display for HitPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.aggregation() {
            Some(a) => write!(f, "{} {}", self.name(), a.name()),
            None => f.write_str(self.name()),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DecisionTable {
    pub id: String,
    pub hit_policy: HitPolicy,
    pub inputs: Vec<InputClause>,
    pub outputs: Vec<OutputClause>,
    pub annotations: Vec<String>,
    pub rules: Vec<Rule>,
}

impl DecisionTable {
    pub fn note<'a>(&self, rule: &'a Rule) -> &'a str {
        if self.annotations.is_empty() { &rule.description } else { rule.annotations.first().map_or("", |s| s.as_str()) }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InputClause {
    pub id: String,
    pub label: String,
    pub expression: String,
    pub type_ref: Option<String>,
    pub lang: ExpressionLanguage,
    pub input_values: Option<String>,
    pub binding: String,
}

impl InputClause {
    pub fn title(&self) -> &str {
        if self.label.is_empty() { &self.expression } else { &self.label }
    }
}

#[derive(Clone, Debug, Default)]
pub struct OutputClause {
    pub id: String,
    pub label: String,
    pub name: String,
    pub type_ref: Option<String>,
    pub output_values: Option<String>,
}

impl OutputClause {
    pub fn title(&self) -> &str {
        if self.label.is_empty() { &self.name } else { &self.label }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub id: String,
    pub text: String,
    pub lang: ExpressionLanguage,
}

impl Entry {
    pub fn is_any(&self) -> bool {
        let t = self.text.trim();
        t.is_empty() || t == "-"
    }
}

#[derive(Clone, Debug, Default)]
pub struct InputEntry {
    pub id: String,
    pub text: String,
    pub lang: ExpressionLanguage,
    pub tests: UnaryTests,
}

impl InputEntry {
    pub fn is_any(&self) -> bool {
        self.tests.is_any()
    }
}

impl Rule {
    pub fn summary(&self) -> String {
        std::iter::once(&self.description).chain(&self.annotations).filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" | ")
    }
}

#[derive(Clone, Debug, Default)]
pub struct Rule {
    pub id: String,
    pub line: u32,
    pub description: String,
    pub annotations: Vec<String>,
    pub inputs: Vec<InputEntry>,
    pub outputs: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Language {
    #[default]
    Feel,
    Js,
    Juel,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LanguageSource {
    Cell,
    FileDefault,
    #[default]
    Default,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpressionLanguage {
    pub language: Language,
    pub source: LanguageSource,
    pub declared: Option<String>,
}

impl ExpressionLanguage {
    pub fn resolve(cell: Option<&str>, file: Option<&str>) -> Self {
        let (declared, source) = match (cell, file) {
            (Some(c), _) => (Some(c), LanguageSource::Cell),
            (None, Some(f)) => (Some(f), LanguageSource::FileDefault),
            (None, None) => (None, LanguageSource::Default),
        };
        let language = match declared.map(|l| l.to_ascii_lowercase()).as_deref() {
            None => Language::Feel,
            Some(l) if l.contains("feel") => Language::Feel,
            Some("juel") => Language::Juel,
            Some("javascript" | "js" | "ecmascript" | "nashorn" | "graal.js") => Language::Js,
            Some(_) => Language::Unsupported,
        };
        ExpressionLanguage { language, source, declared: declared.map(str::to_string) }
    }

    pub fn is_explicit(&self) -> bool {
        self.source == LanguageSource::Cell
    }

    pub fn short_name(&self) -> Cow<'static, str> {
        match self.language {
            Language::Unsupported => Cow::Owned(self.declared.as_deref().unwrap_or("?").to_uppercase()),
            l => Cow::Borrowed(l.short()),
        }
    }

    pub fn long_name(&self) -> Cow<'static, str> {
        match self.language {
            Language::Unsupported => self.short_name(),
            l => Cow::Borrowed(l.long()),
        }
    }

    pub fn describe(&self) -> String {
        if self.is_explicit() { self.long_name().into_owned() } else { format!("{} (default)", self.long_name()) }
    }
}

impl Language {
    pub const CHOICES: [(&'static str, Option<&'static str>); 4] = [("Default (file or FEEL)", None), ("FEEL", Some("feel")), ("JavaScript", Some("javascript")), ("JUEL", Some("juel"))];

    pub fn short(self) -> &'static str {
        match self {
            Language::Feel => "FEEL",
            Language::Js => "JS",
            Language::Juel => "JUEL",
            Language::Unsupported => "?",
        }
    }

    pub fn long(self) -> &'static str {
        match self {
            Language::Js => "JavaScript",
            l => l.short(),
        }
    }
}
