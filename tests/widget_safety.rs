use rhai::{Engine, EvalAltResult, OptimizationLevel};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn bare_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_optimization_level(OptimizationLevel::Simple);
    engine
}

fn must_compile(engine: &Engine, source: &str) -> rhai::AST {
    match engine.compile(source) {
        Ok(ast) => ast,
        Err(err) => panic!("compile failed: {err}"),
    }
}

fn must_run_error(engine: &mut Engine, script: &str) -> Box<EvalAltResult> {
    match engine.run(script) {
        Ok(_) => panic!("expected error for: {script}"),
        Err(err) => err,
    }
}

#[test]
fn meta_nested_literal_extracts() {
    let engine = bare_engine();
    let ast = must_compile(
        &engine,
        r#"const META = #{
            title: "Weather",
            interval: "10m",
            size: "small",
            needs: ["net:http", "time:now"],
            hosts: ["api.open-meteo.com"],
            config: #{
                lat: #{ type: "string", required: true, help: "Latitude" },
                lon: #{ type: "string", required: true, help: "Longitude" },
                api_key: #{ type: "string", secret: true, help: "API key" },
                city: #{ type: "string", fallback: "Ghent", help: "City name" },
                refresh: #{ type: "number", fallback: 10, help: "Minutes" },
                verbose: #{ type: "bool", required: true, help: "Debug output" },
                tags: #{ type: "list", fallback: [], help: "Labels" },
            },
            computed: 40 + 2,
        };
        "#,
    );
    let meta = ast
        .iter_literal_variables(true, false)
        .find(|(name, _, _)| *name == "META")
        .map(|(_, _, value)| value);
    let Some(meta) = meta else {
        panic!("META not extracted");
    };
    let json = match serde_json::to_value(&meta) {
        Ok(json) => json,
        Err(err) => panic!("to_value failed: {err}"),
    };
    assert_eq!(json["title"], "Weather");
    assert_eq!(json["interval"], "10m");
    assert_eq!(json["size"], "small");
    assert_eq!(json["needs"], serde_json::json!(["net:http", "time:now"]));
    assert_eq!(json["hosts"], serde_json::json!(["api.open-meteo.com"]));
    assert_eq!(json["computed"], 42);
    assert_eq!(json["config"]["lat"]["type"], "string");
    assert_eq!(json["config"]["lat"]["required"], true);
    assert_eq!(json["config"]["lat"]["help"], "Latitude");
    assert_eq!(json["config"]["lon"]["type"], "string");
    assert_eq!(json["config"]["api_key"]["secret"], true);
    assert_eq!(json["config"]["city"]["fallback"], "Ghent");
    assert_eq!(json["config"]["refresh"]["fallback"], 10);
    assert_eq!(json["config"]["verbose"]["required"], true);
    assert_eq!(json["config"]["tags"]["fallback"], serde_json::json!([]));
}

#[test]
fn infinite_loop_errors_under_max_operations() {
    let mut engine = bare_engine();
    engine.set_max_operations(1000);
    let err = must_run_error(&mut engine, "while true {}");
    assert!(
        matches!(*err, EvalAltResult::ErrorTooManyOperations(..)),
        "got {err:?}"
    );
}

#[test]
fn map_growth_past_limit_stops_native_calls() {
    for script in [
        r#"let m = #{a:1,b:2,c:3,d:4}; marker(); m["extra"] = 42; marker();"#,
        r#"let m = #{child: #{a:1,b:2,c:3}}; marker(); m["child"]["extra"] = 42; marker();"#,
        r#"let m = #{child: #{a:1,b:2,c:3}}; marker(); m.child.extra = 42; marker();"#,
        r#"let m = #{a:1,b:2,c:3,d:4}; marker(); m.extra = 42; marker();"#,
    ] {
        let hits = Arc::new(AtomicUsize::new(0));
        let mut engine = bare_engine();
        engine.set_max_map_size(4);
        let counter = Arc::clone(&hits);
        engine.register_fn("marker", move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let err = must_run_error(&mut engine, script);
        assert!(
            matches!(*err, EvalAltResult::ErrorDataTooLarge(..)),
            "script: {script} got {err:?}"
        );
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "marker must stop after limit: {script}"
        );
    }
}
