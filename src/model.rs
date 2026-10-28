use crate::unary::UnaryTests;

#[derive(Clone, Debug, Default)]
pub struct Definitions {
    pub id: String,
    pub name: String,
    pub namespace: String,
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

#[derive(Clone, Debug, Default)]
pub struct DecisionTable {
    pub id: String,
    pub hit_policy: String,
    pub aggregation: Option<String>,
    pub inputs: Vec<InputClause>,
    pub outputs: Vec<OutputClause>,
    pub annotations: Vec<String>,
    pub rules: Vec<Rule>,
}

impl DecisionTable {
    /// The rule note the editor changes: the first annotation column when the table has
    /// annotation columns (DMN 1.3), otherwise the rule's description.
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
    /// Description and annotation texts joined for display.
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

    pub fn short_name(&self) -> String {
        match self.language {
            Language::Feel => "FEEL".into(),
            Language::Js => "JS".into(),
            Language::Juel => "JUEL".into(),
            Language::Unsupported => self.declared.as_deref().unwrap_or("?").to_uppercase(),
        }
    }
}
