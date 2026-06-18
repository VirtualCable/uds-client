// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

macro_rules! register_js_module {
    (
        $ctx:expr,
        $mod_name:expr,
        [ $( ($name:expr, $fn:expr, $len:expr) ),* $(,)? ],        // sync fns
        [ $( ($aname:expr, $afn:expr, $alen:expr) ),* $(,)? ]     // async fns
        $(,)?
    ) => {{
        // local helper macro
        macro_rules! register_fn {
            ($obj:expr, $realm:expr, $fn_name:expr, $ctor:expr, $fn_len:expr) => {{
                let fn_obj = boa_engine::object::FunctionObjectBuilder::new(
                        $realm,
                        $ctor,
                    )
                    .name($fn_name)
                    .length($fn_len)
                    .build();

                $obj.insert_property(
                    boa_engine::js_string!($fn_name),
                    boa_engine::property::PropertyDescriptorBuilder::new()
                        .value(fn_obj)
                        .writable(false)
                        .enumerable(false)
                        .configurable(false),
                );
            }};
        }

        let obj = boa_engine::JsObject::with_null_proto();
        let realm = $ctx.realm();

        // Sync functions
        $(
            register_fn!(obj, realm, $name,
                boa_engine::native_function::NativeFunction::from_fn_ptr($fn),
                $len
            );
        )*

        // Async functions
        $(
            register_fn!(obj, realm, $aname,
                boa_engine::native_function::NativeFunction::from_async_fn($afn),
                $alen
            );
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
                            match js_val.try_js_into::<$t>($ctx) {
                                Ok(v) => v,
                                Err(e) => {
                                    shared::log::error!(
                                        "extract_js_args: failed to convert argument {} to {}: {e}",
                                        _i - 1,
                                        stringify!($t),
                                    );
                                    <$t>::default()
                                }
                            }
                        } else {
                            <$t>::default()
                        }
                    }
                ),*
            )
        }
    };
}
