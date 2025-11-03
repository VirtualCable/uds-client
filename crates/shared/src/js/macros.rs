macro_rules! register_js_module {
    ($ctx:expr, $mod_name:expr, [ $( ($name:expr, $fn:expr, $len:expr) ),* $(,)? ]) => {{
        let obj = boa_engine::JsObject::with_null_proto();
        let realm = $ctx.realm();
        $(
            {
                let fn_obj = boa_engine::object::FunctionObjectBuilder::new(
                        realm,
                        boa_engine::native_function::NativeFunction::from_fn_ptr($fn),
                    )
                    .name($name)
                    .length($len)
                    .build();

                obj.insert_property(
                    boa_engine::js_string!($name),
                    boa_engine::property::PropertyDescriptorBuilder::new()
                        .value(fn_obj)
                        .writable(true)
                        .enumerable(true)
                        .configurable(true),
                );
            }
        )*

        $ctx.register_global_property(
            boa_engine::js_string!($mod_name),
            boa_engine::JsValue::from(obj),
            boa_engine::property::Attribute::all(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to register {} object: {}", $mod_name, e))?;
    }};
}

macro_rules! extract_js_args {
    ($args:expr, $ctx:expr, $($t:ty),* $(,)?) => {
        {
            let mut _i = 0;
            (
                $(
                    {
                        let val = $args.get(_i);
                        _i += 1;
                        if let Some(js_val) = val {
                            js_val.try_js_into::<$t>($ctx)
                                .unwrap_or_default()
                        } else {
                            <$t>::default()
                        }
                    }
                ),*
            )
        }
    };
}
