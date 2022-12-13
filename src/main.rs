use deno_core::{v8, Extension, FsModuleLoader, JsRuntime, RuntimeOptions};
use deno_runtime::{js, permissions::Permissions};
use futures::executor::block_on;
use serde::Deserialize;
use std::rc::Rc;

fn new_runtime() -> JsRuntime {
    let extensions: Vec<Extension> = vec![
        deno_web::init::<Permissions>(deno_web::BlobStore::default(), None),
        deno_crypto::init(None),
    ];

    JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(FsModuleLoader)),
        startup_snapshot: Some(js::deno_isolate_init()),
        source_map_getter: None,
        get_error_class_fn: None,
        shared_array_buffer_store: None,
        compiled_wasm_module_store: None,
        extensions,
        ..Default::default()
    })
}

async fn execute_script<'a, T: Deserialize<'a>>(runtime: &mut JsRuntime, code: &str) -> T {
    let value = runtime
        .execute_script(&deno_core::located_script_name!(), code)
        .unwrap();
    let value = runtime.resolve_value(value).await.unwrap();
    let scope = &mut runtime.handle_scope();
    let local_value = v8::Local::new(scope, value);
    serde_v8::from_v8(scope, local_value).unwrap()
}

async fn execute_module(runtime: &mut JsRuntime, code: &str) -> i32 {
    let main_module = deno_core::resolve_path("module.js").unwrap();
    let module_id = runtime
        .load_side_module(&main_module, Some(code.to_string()))
        .await
        .unwrap();
    let _ = runtime.mod_evaluate(module_id);
    runtime.run_event_loop(false).await.unwrap();
    module_id
}

async fn get_module_export<'a, T: Deserialize<'a>>(
    runtime: &mut JsRuntime,
    module_id: i32,
    key: &str,
) -> T {
    let module_handle_scope = runtime.get_module_namespace(module_id).unwrap();
    let global_handle_scope = &mut runtime.handle_scope();
    let local_handle_scope = v8::Local::<v8::Object>::new(global_handle_scope, module_handle_scope);

    let export_name = v8::String::new(global_handle_scope, key).unwrap();
    let binding = local_handle_scope.get(global_handle_scope, export_name.into());
    let object = binding.unwrap();
    let got: T = serde_v8::from_v8(global_handle_scope, object).unwrap();
    got
}

fn main() {
    let mut runtime = new_runtime();
    block_on(async move {
        execute_module(
            &mut runtime,
            "const name = 'gorilla'; globalThis.name = name;",
        )
        .await;
        let result: serde_json::Value = execute_script(
            &mut runtime,
            r#"
'hello ' + name"#,
        )
        .await;
        dbg!(result);
    });
}
