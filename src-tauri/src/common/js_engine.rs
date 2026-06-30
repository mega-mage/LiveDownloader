use boa_engine::{Context, Source, JsValue, JsString};
use std::fs;
use std::path::Path;

pub struct JsEngine {
    context: Context,
}

impl JsEngine {
    pub fn new() -> Self {
        Self {
            context: Context::default(),
        }
    }

    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let content = fs::read_to_string(path)?;
        self.context.eval(Source::from_bytes(&content))
            .map_err(|e| format!("JS Eval Error: {}", e))?;
        Ok(())
    }

    pub fn load_code(&mut self, code: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.context.eval(Source::from_bytes(code))
            .map_err(|e| format!("JS Eval Error: {}", e))?;
        Ok(())
    }

    pub fn call_function(
        &mut self,
        func_name: &str,
        args: &[String],
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let global_obj = self.context.global_object().clone();
        let js_func = global_obj.get(JsString::from(func_name), &mut self.context)
            .map_err(|e| format!("JS Property Error: {}", e))?
            .as_callable()
            .ok_or_else(|| format!("Function '{}' not found or not callable", func_name))?
            .clone();

        let js_args: Vec<JsValue> = args
            .iter()
            .map(|s| JsValue::from(JsString::from(s.as_str())))
            .collect();

        let this_val = JsValue::from(global_obj);
        let result = js_func.call(&this_val, &js_args, &mut self.context)
            .map_err(|e| format!("JS Call Error: {}", e))?;
            
        let result_str = result.as_string()
            .ok_or("Result is not a string")?
            .to_std_string()
            .map_err(|e| format!("UTF-16 conversion error: {}", e))?;
            
        Ok(result_str)
    }
}
