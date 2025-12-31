use bytecode_vm::{NativeFunction, Value};
use js_sys::{Array, Date};
use wasm_bindgen::prelude::*;
use bytecode_vm::interpreter::Interpreter;
use bytecode_vm::interpreter::{CompilerError, RuntimeError};

#[wasm_bindgen]
pub struct WasmVm {
    interpreter: Interpreter
}

#[wasm_bindgen]
pub struct CompilerErr {
    pub line: usize,
    pub start: usize,
    pub len: usize,
    message: String
}

#[wasm_bindgen]
impl CompilerErr {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }
}

#[wasm_bindgen]
pub struct JsNativeFn {
    name: String,
    arity: u8,
    function: js_sys::Function
}

#[wasm_bindgen]
impl JsNativeFn {
    #[wasm_bindgen(constructor)]
    pub fn new(name: String, arity: u8, function: js_sys::Function) -> JsNativeFn
    {
        return JsNativeFn {
            name,
            arity,
            function
        }
    }
}

#[wasm_bindgen]
pub struct CompileResult {
    success: bool,
    vm: Option<WasmVm>,
    compile_errors: Option<Vec<CompilerErr>>,
}

#[wasm_bindgen]
impl CompileResult {
    #[wasm_bindgen(getter)]
    pub fn success(&self) -> bool {
        self.success
    }
    #[wasm_bindgen]
    pub fn take_interpreter(&mut self) -> Option<WasmVm> {
        return self.vm.take();
    }
    #[wasm_bindgen]
    pub fn take_compile_errors(&mut self) -> Option<Vec<CompilerErr>> {
        self.compile_errors.take()
    }
}

impl CompileResult {
    fn new_success(interpreter: Interpreter) -> Self {
        Self {
            success: true,
            vm: Some(WasmVm { interpreter }),
            compile_errors: None,
        }
    }
    fn new_failure(errors: Vec<CompilerError>) -> Self {
        Self {
            success: false,
            vm: None,
            compile_errors: Some(errors.into_iter().map(|i| { CompilerErr {
                line: i.line,
                start: i.start,
                len: i.len,
                message: i.message
            }}).collect()),
        }
    }
}

#[wasm_bindgen]
pub struct Output {
    finished: bool,
    runtime_error: Option<String>
}

#[wasm_bindgen]
impl Output {
    #[wasm_bindgen(getter)]
    pub fn finished(&self) -> bool {
        self.finished
    }

    #[wasm_bindgen(getter)]
    pub fn runtime_error(&self) -> Option<String> {
        self.runtime_error.clone()
    }
}

impl Output {
    pub fn successful() -> Self {
        return Self {
            finished: true,
            runtime_error: None
        };
    }
    pub fn runtime_err(err: RuntimeError) -> Self {
        return Self {
            finished: true,
            runtime_error: Some(err.message.to_owned())
        };
    }
    pub fn unfinished() -> Self {
        return Self {
            finished: false,
            runtime_error: None
        };
    }
}

#[wasm_bindgen]
pub fn compile(source: &str, natives: Vec<JsNativeFn>) -> CompileResult {
    let mut rust_natives: Vec::<NativeFunction> = vec![];
    for native in natives.into_iter() {
        rust_natives.push(native.into_native());
    }
    let time = NativeFunction {
        name: "time".to_owned(),
        arity: 0,
        function: {
            fn time(_: &[Value]) -> Value {
                let millis = Date::now();
                Value::Number(millis / 1000.0)
            }
            Box::new(time)
        },
    };
    rust_natives.push(time);

    return match Interpreter::new(source.to_owned(), rust_natives) {
        Ok(interpreter) => CompileResult::new_success(interpreter),
        Err(compiler_errors) => CompileResult::new_failure(compiler_errors),
    };
}

#[wasm_bindgen]
impl WasmVm {
    #[wasm_bindgen]
    pub fn interpret(&mut self) -> Output {
        return match self.interpreter.run() {
            Ok(_) => Output::successful(),
            Err(runtime_error) => Output::runtime_err(runtime_error),
        };
    }
    #[wasm_bindgen]
    pub fn step(&mut self) -> Output {
        return match self.interpreter.step() {
            Ok(not_finished) => {
                if not_finished { Output::unfinished() }
                else { Output::successful() }
            },
            Err(runtime_error) => Output::runtime_err(runtime_error),
        };
    }
}

pub trait IntoNative { 
    fn into_native(self) -> NativeFunction; 
}

pub trait JsConvert { 
    fn to_js(&self) -> JsValue; 
    fn from_js(js: JsValue) -> Self; 
}

impl IntoNative for JsNativeFn {
    fn into_native(self) -> NativeFunction {
        let js_func = self.function;

        NativeFunction {
            name: self.name,
            arity: self.arity,
            function: Box::new(move |vals: &[Value]| {

                let js_args = vals.iter()
                    .map(|v| v.to_js())
                    .collect::<Vec<_>>();

                let array = Array::new();
                for arg in js_args {
                    array.push(&arg);
                }

                let result = js_func
                    .apply(&JsValue::NULL, &array)
                    .expect("JS function threw");

                Value::from_js(result)
            }),
        }
    }
}

impl JsConvert for Value {
    fn to_js(&self) -> JsValue {
        match self {
            Value::Number(n) => JsValue::from_f64(*n),
            Value::Bool(b) => JsValue::from_bool(*b),
            Value::String(s) => JsValue::from_str(s),
            Value::Null => JsValue::NULL,
            _ => panic!("Unsupported value.")
        }
    }

    fn from_js(js: JsValue) -> Self {
        if js.is_null() || js.is_undefined() {
            Value::Null
        } else if let Some(n) = js.as_f64() {
            Value::Number(n)
        } else if let Some(b) = js.as_bool() {
            Value::Bool(b)
        } else if let Some(s) = js.as_string() {
            Value::String(s.into())
        } else {
            panic!("Unsupported JS value")
        }
    }
}

