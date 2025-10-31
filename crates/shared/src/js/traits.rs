use boa_engine::JsValue;

pub trait FromJsValue: Sized + Default {
    fn from_js(v: Option<&JsValue>) -> Self;
}

impl FromJsValue for String {
    fn from_js(v: Option<&JsValue>) -> Self {
        v.and_then(JsValue::as_string)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default()
    }
}

impl FromJsValue for u16 {
    fn from_js(v: Option<&JsValue>) -> Self {
        v.and_then(JsValue::as_number)
            .map(|n| n as u16)
            .unwrap_or_default()
    }
}

impl FromJsValue for u64 {
    fn from_js(v: Option<&JsValue>) -> Self {
        v.and_then(JsValue::as_number)
            .map(|n| n as u64)
            .unwrap_or_default()
    }
}

// etc. para otros tipos
