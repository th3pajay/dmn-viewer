use crate::model::*;
use crate::unary::UnaryTests;
use roxmltree::{Document, Node};
use std::collections::HashMap;

pub(crate) fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

pub(crate) fn kids<'a, 'i: 'a>(n: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children().filter(move |c| c.is_element() && c.tag_name().name() == name)
}

fn text_child(n: Node, name: &str) -> String {
    child(n, name).map(|t| t.text().unwrap_or("").to_string()).unwrap_or_default()
}

fn attr(n: Node, name: &str) -> String {
    n.attribute(name).unwrap_or("").to_string()
}

fn opt_attr(n: Node, name: &str) -> Option<String> {
    n.attribute(name).map(|s| s.to_string()).filter(|s| !s.is_empty())
}

fn lang(n: Node, file: Option<&str>) -> ExpressionLanguage {
    ExpressionLanguage::resolve(opt_attr(n, "expressionLanguage").as_deref(), file)
}

fn entry(n: Node, file: Option<&str>) -> Entry {
    Entry { id: attr(n, "id"), text: text_child(n, "text"), lang: lang(n, file) }
}

fn input_entry(n: Node, file: Option<&str>) -> InputEntry {
    let Entry { id, text, lang } = entry(n, file);
    let tests = UnaryTests::parse(&text, &lang);
    InputEntry { id, text, lang, tests }
}

fn named(n: Node) -> NamedElement {
    NamedElement { id: attr(n, "id"), name: attr(n, "name") }
}

pub fn parse(xml: &str) -> Result<Definitions, String> {
    let xml = xml.trim_start_matches('\u{feff}');
    let doc = Document::parse(xml).map_err(|e| format!("XML error: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "definitions" {
        return Err(format!("Root element is <{}>, expected <definitions>", root.tag_name().name()));
    }

    let mut seen: HashMap<&str, usize> = HashMap::new();
    for n in root.descendants().filter(|n| n.is_element()) {
        if let Some(id) = n.attribute("id") {
            *seen.entry(id).or_default() += 1;
        }
    }
    let mut duplicate_ids: Vec<String> = seen.into_iter().filter(|(_, c)| *c > 1).map(|(k, _)| k.to_string()).collect();
    duplicate_ids.sort();

    let mut defs = Definitions {
        id: attr(root, "id"),
        name: attr(root, "name"),
        namespace: attr(root, "namespace"),
        exporter: attr(root, "exporter"),
        exporter_version: attr(root, "exporterVersion"),
        expression_language: opt_attr(root, "expressionLanguage"),
        duplicate_ids,
        ..Default::default()
    };

    let file = defs.expression_language.clone();
    let file = file.as_deref();
    let mut lines = LineCursor { text: xml, pos: 0, row: 1 };
    defs.input_data = kids(root, "inputData").map(named).collect();
    defs.knowledge_sources = kids(root, "knowledgeSource").map(named).collect();
    defs.bkms = kids(root, "businessKnowledgeModel").map(named).collect();

    for d in kids(root, "decision") {
        let attributes = d.attributes().filter(|a| !matches!(a.name(), "id" | "name")).map(|a| (a.name().to_string(), a.value().to_string())).collect();

        let requirements = d
            .children()
            .filter(|c| c.is_element() && c.tag_name().name().ends_with("Requirement"))
            .flat_map(|r| r.children().filter(|x| x.is_element()))
            .map(|x| Requirement { kind: x.tag_name().name().to_string(), href: attr(x, "href").trim_start_matches('#').to_string() })
            .collect();

        let logic = if let Some(t) = child(d, "decisionTable") {
            DecisionLogic::Table(parse_table(&mut lines, t, file))
        } else if let Some(l) = child(d, "literalExpression") {
            DecisionLogic::Literal { text: text_child(l, "text"), lang: lang(l, file), variable: child(d, "variable").map(|v| attr(v, "name")) }
        } else {
            DecisionLogic::None
        };

        defs.decisions.push(Decision { id: attr(d, "id"), name: attr(d, "name"), attributes, requirements, logic });
    }
    Ok(defs)
}

// Rules arrive in document order, so counting forward stays linear; text_pos_at rescans from byte 0 each call.
struct LineCursor<'a> {
    text: &'a str,
    pos: usize,
    row: u32,
}

impl LineCursor<'_> {
    fn row_at(&mut self, pos: usize) -> u32 {
        let pos = pos.min(self.text.len());
        if pos < self.pos {
            self.pos = 0;
            self.row = 1;
        }
        self.row += self.text.as_bytes()[self.pos..pos].iter().filter(|&&b| b == b'\n').count() as u32;
        self.pos = pos;
        self.row
    }
}

fn parse_table(lines: &mut LineCursor, t: Node, file: Option<&str>) -> DecisionTable {
    let inputs = kids(t, "input")
        .map(|i| {
            let ie = child(i, "inputExpression");
            InputClause {
                id: attr(i, "id"),
                label: attr(i, "label"),
                expression: ie.map(|e| text_child(e, "text")).unwrap_or_default(),
                type_ref: ie.and_then(|e| opt_attr(e, "typeRef")),
                lang: ExpressionLanguage::resolve(ie.and_then(|e| opt_attr(e, "expressionLanguage")).as_deref(), file),
                input_values: child(i, "inputValues").map(|v| text_child(v, "text")),
            }
        })
        .collect();

    let outputs = kids(t, "output")
        .map(|o| OutputClause {
            id: attr(o, "id"),
            label: attr(o, "label"),
            name: attr(o, "name"),
            type_ref: opt_attr(o, "typeRef"),
            output_values: child(o, "outputValues").map(|v| text_child(v, "text")),
        })
        .collect();

    let rules = kids(t, "rule")
        .map(|r| Rule {
            id: attr(r, "id"),
            line: lines.row_at(r.range().start),
            description: text_child(r, "description"),
            annotations: kids(r, "annotationEntry").map(|a| text_child(a, "text")).collect(),
            inputs: kids(r, "inputEntry").map(|n| input_entry(n, file)).collect(),
            outputs: kids(r, "outputEntry").map(|n| entry(n, file)).collect(),
        })
        .collect();

    DecisionTable {
        id: attr(t, "id"),
        hit_policy: opt_attr(t, "hitPolicy").unwrap_or_else(|| "UNIQUE".into()),
        aggregation: opt_attr(t, "aggregation"),
        inputs,
        outputs,
        annotations: kids(t, "annotation").map(|a| attr(a, "name")).collect(),
        rules,
    }
}
