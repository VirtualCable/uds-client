// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

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
