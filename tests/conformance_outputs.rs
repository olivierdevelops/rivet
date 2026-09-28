//! T-20 — declared outputs.

use rivet::internal::domain::outputs::ValueSpec;
use rivet::internal::domain::ports::Parser;
use rivet::internal::domain::source::SourceFile;
use rivet::internal::features::language::compile_output_spec::compile_output_spec;
use rivet::internal::infra::capy_parser::CapyParser;

// vhco:test language.compile_output_spec -- a nested object output keeps field types, optionality and descriptions
#[test]
fn nested_object_output() {
    let text = "operation a.b\n    output object description \"User.\"\n        field id integer required description \"ID.\"\n        field address object optional\n            field city text\n        end\n    end\n    return {}\nend\n";
    let tree = CapyParser::new()
        .unwrap()
        .parse(&SourceFile {
            path: "t".into(),
            text: text.into(),
        })
        .unwrap();
    let out_node = tree.nodes[0]
        .children()
        .iter()
        .find(|n| n.func == "output_block")
        .unwrap();
    let spec = compile_output_spec(out_node).unwrap();
    assert_eq!(spec.description.as_deref(), Some("User."));
    let ValueSpec::Object { fields, open } = spec.spec else {
        panic!()
    };
    assert!(!open);
    assert_eq!(fields[0].name, "id");
    assert!(!fields[1].required);
    assert!(
        matches!(&fields[1].spec, ValueSpec::Object { fields, .. } if fields[0].name == "city")
    );
}
