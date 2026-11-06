use eventabi_guard::{
    abi, consumer,
    diff::{self, Category, Certainty},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

fn input(name: &str, kind: &str, indexed: bool) -> Value {
    json!({"name":name,"type":kind,"indexed":indexed})
}
fn event(name: &str, inputs: Vec<Value>) -> Value {
    json!({"type":"event","name":name,"inputs":inputs,"anonymous":false})
}
fn parse(events: Vec<Value>) -> abi::ContractAbi {
    abi::parse(&json!(events).to_string()).unwrap()
}
fn basic() -> Value {
    event(
        "Payment",
        vec![
            input("payer", "address", true),
            input("amount", "uint256", false),
        ],
    )
}
struct Fixture(PathBuf);
impl Fixture {
    fn new(old: Value, new: Value) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "eventabi-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        for (n, v) in [("old", old), ("new", new)] {
            std::fs::write(path.join(format!("{n}.json")), json!([v]).to_string()).unwrap();
        }
        Self(path)
    }
    fn load(
        &self,
        fields: &str,
        mode: &str,
    ) -> eventabi_guard::Result<consumer::ValidatedManifest> {
        let yaml = format!(
            "version: 1\ncontracts:\n  - id: C\n    old: old.json\n    new: new.json\nconsumers:\n  - id: app\n    contract: C\n    events:\n      - signature: Payment(address,uint256)\n        mode: {mode}\n{fields}"
        );
        self.yaml(&yaml)
    }
    fn yaml(&self, yaml: &str) -> eventabi_guard::Result<consumer::ValidatedManifest> {
        let path = self.0.join("manifest.yaml");
        std::fs::write(&path, yaml).unwrap();
        consumer::load(&path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn known_ethereum_transfer_vector() {
    let abi = parse(vec![event(
        "Transfer",
        vec![
            input("from", "address", true),
            input("to", "address", true),
            input("value", "uint256", false),
        ],
    )]);
    assert_eq!(
        abi.events.values().next().unwrap().topic0.as_deref(),
        Some("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
    );
}
#[test]
fn keccak_empty_vector() {
    assert_eq!(
        abi::keccak(""),
        "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    );
}
#[test]
fn raw_artifact_empty_functions() {
    for v in [
        json!([]),
        json!({"abi":[],"bytecode":{"object":"00"},"extra":true}),
        json!([{"type":"function","name":"f"}]),
    ] {
        assert!(abi::parse(&v.to_string()).unwrap().events.is_empty());
    }
    let e = basic();
    assert_eq!(
        abi::parse(&json!([e.clone()]).to_string()).unwrap(),
        abi::parse(&json!({"abi":[e],"metadata":123}).to_string()).unwrap()
    );
}
#[test]
fn malformed_input_shapes() {
    for text in [
        "{",
        "null",
        "{}",
        "1",
        "{\"abi\":null}",
        "[{}]",
        "[1]",
        "[{\"type\":\"evnt\"}]",
    ] {
        assert!(abi::parse(text).is_err(), "{text}");
    }
}
#[test]
fn required_event_properties() {
    for key in ["name", "inputs", "anonymous"] {
        let mut e = basic();
        e.as_object_mut().unwrap().remove(key);
        assert!(abi::parse(&json!([e]).to_string()).is_err());
    }
}
#[test]
fn malformed_types() {
    for t in [
        "uint7",
        "uint257",
        "bytes0",
        "bytes33",
        "address payable",
        "tuple",
        "uint256[0]",
        "uint256[01]",
        "uint256[",
        "uint256[]x",
        "uint08",
        "mapping",
        "uint256[18446744073709551616]",
    ] {
        assert!(
            abi::parse(&json!([event("E", vec![input("x", t, false)])]).to_string()).is_err(),
            "{t}"
        );
    }
}
#[test]
fn aliases_and_arrays() {
    let a = parse(vec![event(
        "E",
        vec![
            input("a", "uint", false),
            input("b", "int[][3]", false),
            input("c", "bytes32[2][]", false),
        ],
    )]);
    assert!(a.events.contains_key("E(uint256,int256[][3],bytes32[2][])"));
}
#[test]
fn tuple_nested_arrays() {
    let t = json!({"name":"data","type":"tuple[][2]","indexed":false,"components":[{"name":"owner","type":"address"},{"name":"nested","type":"tuple[]","components":[{"name":"n","type":"uint"}]}]});
    let a = parse(vec![event("E", vec![t])]);
    assert!(a.events.contains_key("E((address,(uint256)[])[][2])"));
}
#[test]
fn anonymous_no_emitted_topic() {
    let mut e = basic();
    e["anonymous"] = json!(true);
    let a = parse(vec![e]);
    assert_eq!(a.events.values().next().unwrap().topic0, None);
}
#[test]
fn names_and_indexing_do_not_affect_hash() {
    let a = parse(vec![basic()]);
    let mut e = basic();
    e["inputs"][0]["name"] = json!("customer");
    e["inputs"][0]["indexed"] = json!(false);
    let b = parse(vec![e]);
    assert_eq!(
        a.events.keys().collect::<Vec<_>>(),
        b.events.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        a.events.values().next().unwrap().topic0,
        b.events.values().next().unwrap().topic0
    );
}
#[test]
fn duplicate_and_overload() {
    let e = basic();
    assert!(abi::parse(&json!([e.clone(), e]).to_string()).is_err());
    let a = parse(vec![
        basic(),
        event("Payment", vec![input("x", "bytes32", false)]),
    ]);
    assert_eq!(a.events.len(), 2);
}
#[test]
fn indexed_limits() {
    let inputs = vec![input("a", "uint", true); 4];
    assert!(abi::parse(&json!([event("E", inputs.clone())]).to_string()).is_err());
    let mut e = event("E", inputs);
    e["anonymous"] = json!(true);
    assert!(abi::parse(&json!([e]).to_string()).is_ok());
}
#[test]
fn missing_file() {
    assert!(abi::load(std::path::Path::new("/nonexistent/eventabi.json")).is_err());
}
#[test]
fn signatures_validate() {
    for s in ["E()", "E(address,uint256)", "E((address,(uint256)[])[][2])"] {
        abi::validate_signature(s).unwrap();
    }
    for s in [
        "",
        "E(uint)",
        "E(uint257)",
        "E(address,)",
        "E((address)",
        "E(address))",
        "E(uint256[0])",
        " E()",
        "E(bytes01)",
    ] {
        assert!(abi::validate_signature(s).is_err(), "{s}");
    }
}
#[test]
fn no_diff() {
    let a = parse(vec![basic()]);
    assert!(diff::compare_abis(&a, &a).is_empty());
}
#[test]
fn definite_add_remove() {
    let a = parse(vec![basic()]);
    let b = parse(vec![event("Other", vec![])]);
    let d = diff::compare_abis(&a, &b);
    assert_eq!(d.len(), 2);
    assert_eq!(d[0].category, Category::EventRemoved);
    assert_eq!(d[1].category, Category::EventAdded);
}
#[test]
fn possible_signature_and_type_change() {
    let a = parse(vec![basic()]);
    let mut e = basic();
    e["inputs"][1]["type"] = json!("bytes32");
    let d = diff::compare_abis(&a, &parse(vec![e]));
    assert!(
        d.iter()
            .any(|c| c.category == Category::EventParameterTypeChanged
                && c.certainty == Certainty::Possible)
    );
    assert!(
        d.iter()
            .any(|c| c.category == Category::EventRemoved && c.certainty == Certainty::Definite)
    );
}
#[test]
fn added_removed_parameters() {
    let a = parse(vec![basic()]);
    let mut e = basic();
    e["inputs"]
        .as_array_mut()
        .unwrap()
        .push(input("ref", "bytes32", false));
    let b = parse(vec![e]);
    assert!(
        diff::compare_abis(&a, &b)
            .iter()
            .any(|c| c.category == Category::EventParameterAdded)
    );
    assert!(
        diff::compare_abis(&b, &a)
            .iter()
            .any(|c| c.category == Category::EventParameterRemoved)
    );
}
#[test]
fn ambiguous_overloads_not_paired() {
    let a = parse(vec![
        basic(),
        event("Payment", vec![input("x", "bool", false)]),
    ]);
    let b = parse(vec![event("Payment", vec![input("x", "bytes32", false)])]);
    assert!(
        diff::compare_abis(&a, &b)
            .iter()
            .all(|c| c.certainty == Certainty::Definite)
    );
}
#[test]
fn reorder_unique_fields() {
    let a = parse(vec![event(
        "E",
        vec![input("a", "uint", false), input("b", "uint", false)],
    )]);
    let b = parse(vec![event(
        "E",
        vec![input("b", "uint", false), input("a", "uint", false)],
    )]);
    assert!(
        diff::compare_abis(&a, &b)
            .iter()
            .any(|c| c.category == Category::EventParameterReordered)
    );
}
#[test]
fn anonymous_change() {
    let a = parse(vec![basic()]);
    let mut e = basic();
    e["anonymous"] = json!(true);
    assert!(
        diff::compare_abis(&a, &parse(vec![e]))
            .iter()
            .any(|c| c.category == Category::EventAnonymousChanged)
    );
}
#[test]
fn tuple_component_name_diff() {
    let t = json!({"name":"p","type":"tuple","indexed":false,"components":[{"name":"x","type":"uint256"}]});
    let mut u = t.clone();
    u["components"][0]["name"] = json!("y");
    let d = diff::compare_abis(
        &parse(vec![event("E", vec![t])]),
        &parse(vec![event("E", vec![u])]),
    );
    assert!(d.iter().any(
        |c| c.field.as_deref() == Some("0.0") && c.category == Category::EventParameterRenamed
    ));
}
#[test]
fn decoded_layout_breaks_undeclared_fields() {
    let mut e = basic();
    e["inputs"][0]["indexed"] = json!(false);
    let f = Fixture::new(basic(), e);
    let r = consumer::check(
        &f.load("        fields:\n          - name: amount", "decoded")
            .unwrap(),
    );
    assert!(r.findings.iter().any(|f| f.rule == "EAG003"));
}
#[test]
fn topic_only_ignores_indexed_layout() {
    let mut e = basic();
    e["inputs"][0]["indexed"] = json!(false);
    let f = Fixture::new(basic(), e);
    assert_eq!(
        consumer::check(&f.load("", "topic_only").unwrap())
            .summary
            .breaking,
        0
    );
}
#[test]
fn named_rename_breaks_positional_safe() {
    let mut e = basic();
    e["inputs"][0]["name"] = json!("customer");
    let f = Fixture::new(basic(), e);
    assert_eq!(
        consumer::check(
            &f.load("        fields:\n          - name: payer", "decoded")
                .unwrap()
        )
        .summary
        .breaking,
        1
    );
    assert_eq!(
        consumer::check(
            &f.load("        fields:\n          - position: 0", "decoded")
                .unwrap()
        )
        .summary
        .breaking,
        0
    );
}
#[test]
fn both_name_and_position_enforced() {
    let mut e = basic();
    e["inputs"][0]["name"] = json!("customer");
    let f = Fixture::new(basic(), e);
    assert_eq!(
        consumer::check(
            &f.load(
                "        fields:\n          - position: 0\n            name: payer",
                "decoded"
            )
            .unwrap()
        )
        .summary
        .breaking,
        1
    );
}
#[test]
fn topic_becomes_anonymous_breaks() {
    let mut e = basic();
    e["anonymous"] = json!(true);
    let f = Fixture::new(basic(), e);
    assert!(
        consumer::check(&f.load("", "topic_only").unwrap())
            .findings
            .iter()
            .any(|f| f.rule == "EAG008")
    );
}
#[test]
fn missing_required_signature_breaks() {
    let f = Fixture::new(basic(), event("Other", vec![]));
    assert!(
        consumer::check(&f.load("", "decoded").unwrap())
            .findings
            .iter()
            .any(|f| f.rule == "EAG001")
    );
}
#[test]
fn invalid_fields() {
    let f = Fixture::new(basic(), basic());
    for fields in [
        "          - position: 99",
        "          - name: missing",
        "          - name: ''",
        "          - {}",
        "          - name: payer\n            type: bool",
        "          - name: payer\n            indexed: false",
        "          - name: payer\n          - position: 0",
    ] {
        assert!(
            f.load(&format!("        fields:\n{fields}"), "decoded")
                .is_err(),
            "{fields}"
        );
    }
    assert!(
        f.load("        fields:\n          - name: payer", "topic_only")
            .is_err()
    );
}
#[test]
fn invalid_manifest_baseline_refs() {
    let f = Fixture::new(basic(), basic());
    let base = "version: 1\ncontracts:\n  - id: C\n    old: old.json\n    new: new.json\nconsumers:\n  - id: app\n    contract: C\n    events:\n      - signature: Payment(address,uint256)\n        mode: decoded\n";
    for bad in [
        base.replace("version: 1", "version: 2"),
        base.replace("contract: C", "contract: Missing"),
        base.replace("Payment(address,uint256)", "Missing()"),
        base.replace("mode: decoded", "mode: magic"),
        base.replace("old: old.json", "old: absent.json"),
        format!("{base}unexpected: 1\n"),
    ] {
        assert!(f.yaml(&bad).is_err());
    }
}
#[test]
fn report_deterministic_formats() {
    let f = Fixture::new(basic(), event("Other", vec![]));
    let v = f.load("", "decoded").unwrap();
    let a = consumer::check(&v);
    let b = consumer::check(&v);
    let text = serde_json::to_string(&a).unwrap();
    assert_eq!(text, serde_json::to_string(&b).unwrap());
    let json: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["summary"]["breaking"], 1);
    for finding in &a.findings {
        assert!(a.human().contains(&finding.rule));
        assert!(a.markdown().contains(&finding.rule));
        assert!(a.github().contains(&finding.rule));
        assert!(finding.id.starts_with("0x"));
    }
}
#[test]
fn input_key_and_event_order_invariant() {
    let a = parse(vec![basic(), event("E", vec![])]);
    let b = parse(vec![event("E", vec![]), basic()]);
    assert_eq!(a, b);
    assert!(diff::compare_abis(&a, &b).is_empty());
}
#[test]
fn bounded_nesting() {
    let mut t = json!({"name":"x","type":"uint256"});
    for _ in 0..26 {
        t = json!({"name":"x","type":"tuple","components":[t]});
    }
    t["indexed"] = json!(false);
    assert!(abi::parse(&json!([event("E", vec![t])]).to_string()).is_err());
}
#[test]
fn reordered_indexed_fields_still_report_positional_layout() {
    let a = parse(vec![event(
        "E",
        vec![input("a", "uint", true), input("b", "uint", false)],
    )]);
    let b = parse(vec![event(
        "E",
        vec![input("b", "uint", false), input("a", "uint", true)],
    )]);
    assert_eq!(
        diff::compare_abis(&a, &b)
            .iter()
            .filter(|c| c.category == Category::EventIndexedChanged)
            .count(),
        2
    );
}
#[test]
fn unconsumed_removal_never_consumer_breakage() {
    let f = Fixture::new(basic(), event("Other", vec![]));
    let v=f.yaml("version: 1\ncontracts:\n  - id: C\n    old: old.json\n    new: new.json\nconsumers: []\n").unwrap();
    assert_eq!(consumer::check(&v).summary.breaking, 0);
}
#[test]
fn duplicate_contract_consumer_and_event_rejected() {
    let f = Fixture::new(basic(), basic());
    let contract = "  - id: C\n    old: old.json\n    new: new.json\n";
    let dependency = "      - signature: Payment(address,uint256)\n        mode: decoded\n";
    let cons = format!("  - id: app\n    contract: C\n    events:\n{dependency}");
    for yaml in [
        format!("version: 1\ncontracts:\n{contract}{contract}consumers: []"),
        format!("version: 1\ncontracts:\n{contract}consumers:\n{cons}{cons}"),
        format!("version: 1\ncontracts:\n{contract}consumers:\n{cons}{dependency}"),
    ] {
        assert!(f.yaml(&yaml).is_err());
    }
}
#[test]
fn ambiguous_and_unnamed_baseline_fields_rejected() {
    let mut a = basic();
    a["inputs"][0]["name"] = json!("amount");
    let f = Fixture::new(a.clone(), a);
    assert!(
        f.load("        fields:\n          - name: amount", "decoded")
            .is_err()
    );
    let mut a = basic();
    a["inputs"][0]["name"] = json!("");
    let f = Fixture::new(a.clone(), a);
    assert!(
        f.load("        fields:\n          - name: payer", "decoded")
            .is_err()
    );
    assert!(
        f.load("        fields:\n          - position: 0", "decoded")
            .is_ok()
    );
}
#[test]
fn named_position_change_detected_with_same_signature() {
    let a = event(
        "Payment",
        vec![input("a", "uint", false), input("b", "uint", false)],
    );
    let b = event(
        "Payment",
        vec![input("b", "uint", false), input("a", "uint", false)],
    );
    let f = Fixture::new(a, b);
    let v=f.yaml("version: 1\ncontracts:\n  - id: C\n    old: old.json\n    new: new.json\nconsumers:\n  - id: app\n    contract: C\n    events:\n      - signature: Payment(uint256,uint256)\n        mode: decoded\n        fields:\n          - name: a\n").unwrap();
    assert!(
        consumer::check(&v)
            .findings
            .iter()
            .any(|f| f.rule == "EAG007")
    );
}
#[test]
fn tuple_type_changes_signature_and_name_changes_do_not() {
    let a = json!({"name":"t","type":"tuple[]","indexed":false,"components":[{"name":"x","type":"uint256"}]});
    let mut b = a.clone();
    b["components"][0]["type"] = json!("address");
    let x = parse(vec![event("E", vec![a.clone()])]);
    let y = parse(vec![event("E", vec![b])]);
    assert_ne!(x.events.keys().next(), y.events.keys().next());
    let mut b = a;
    b["components"][0]["name"] = json!("y");
    let z = parse(vec![event("E", vec![b])]);
    assert_eq!(x.events.keys().next(), z.events.keys().next());
}
#[test]
fn annotation_and_markdown_injection_escaped() {
    let f = Fixture::new(basic(), event("Other", vec![]));
    let yaml = "version: 1\ncontracts:\n  - id: '<script>_*[x](url)'\n    old: old.json\n    new: new.json\nconsumers:\n  - id: \"evil\\n::error::injected%\"\n    contract: '<script>_*[x](url)'\n    events:\n      - signature: Payment(address,uint256)\n        mode: decoded\n";
    let r = consumer::check(&f.yaml(yaml).unwrap());
    let github = r.github();
    assert_eq!(github.lines().count(), r.findings.len());
    assert!(github.contains("evil%0A::error::injected%25"));
    assert!(!r.markdown().contains("<script>"));
    assert!(!r.markdown().contains("[x](url)"));
}
#[test]
fn topic_only_cannot_bind_anonymous_baseline() {
    let mut a = basic();
    a["anonymous"] = json!(true);
    let f = Fixture::new(a.clone(), a);
    assert!(f.load("", "topic_only").is_err());
}
#[test]
fn json_object_key_order_does_not_change_result() {
    let a = abi::parse(r#"[{"type":"event","name":"E","inputs":[],"anonymous":false}]"#).unwrap();
    let b = abi::parse(r#"[{"anonymous":false,"inputs":[],"name":"E","type":"event"}]"#).unwrap();
    assert_eq!(a, b);
}
#[test]
fn unrelated_addition_preserves_all_consumers() {
    for name in ["A", "AnotherEvent", "PaymentExtra"] {
        let f = Fixture::new(basic(), basic());
        std::fs::write(
            f.0.join("new.json"),
            json!([basic(), event(name, vec![])]).to_string(),
        )
        .unwrap();
        let r = consumer::check(
            &f.load("        fields:\n          - name: payer", "decoded")
                .unwrap(),
        );
        assert_eq!(r.summary.breaking, 0);
        assert_eq!(r.summary.info, 1);
    }
}
#[test]
fn parameter_names_never_change_signature_property() {
    for name in ["a", "customer", "value_123", "$valid", ""] {
        let mut b = basic();
        b["inputs"][0]["name"] = json!(name);
        let a = parse(vec![basic()]);
        let b = parse(vec![b]);
        assert_eq!(
            a.events.values().next().unwrap().topic0,
            b.events.values().next().unwrap().topic0
        );
    }
}
#[test]
fn yaml_duplicate_keys_and_negative_positions_rejected() {
    let f = Fixture::new(basic(), basic());
    assert!(
        f.yaml("version: 1\nversion: 1\ncontracts: []\nconsumers: []")
            .is_err()
    );
    assert!(
        f.load("        fields:\n          - position: -1", "decoded")
            .is_err()
    );
}
#[test]
fn tuple_required_component_rename_breaks_named_not_positional() {
    let t = json!({"name":"data","type":"tuple","indexed":false,"components":[{"name":"value","type":"uint256"}]});
    let mut u = t.clone();
    u["components"][0]["name"] = json!("amount");
    let f = Fixture::new(event("E", vec![t]), event("E", vec![u]));
    let base = "version: 1\ncontracts:\n  - id: C\n    old: old.json\n    new: new.json\nconsumers:\n  - id: app\n    contract: C\n    events:\n      - signature: E((uint256))\n        mode: decoded\n        fields:\n          - name: data\n";
    let named = consumer::check(&f.yaml(base).unwrap());
    assert!(named.findings.iter().any(|f| f.rule == "EAG006"));
    let positional = consumer::check(&f.yaml(&base.replace("name: data", "position: 0")).unwrap());
    assert_eq!(positional.summary.breaking, 0);
}
