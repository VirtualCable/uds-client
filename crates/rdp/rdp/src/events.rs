// Defines the event handling mechanism for RDP events using FreeRDP's PubSub system.
#[macro_export]
macro_rules! define_event {
    ($name:ident, $args_ty:ty, $handler_ty:ty) => {
        pub struct $name;

        impl $name {
            #[allow(dead_code)]
            #[allow(clippy::manual_c_str_literals)]
            /// # Safety
            /// Interoperability with C code.
            /// Ensure that the pointers are valid.
            pub unsafe fn fire_event(
                pubsub: *mut freerdp_sys::wPubSub,
                context: *mut std::ffi::c_void,
                e: *const $args_ty,
            ) -> i32 {
                unsafe {
                    freerdp_sys::PubSub_OnEvent(
                        pubsub,
                        concat!(stringify!($name), "\0").as_ptr() as *const i8,
                        context,
                        &(*e).e,
                    )
                }
            }

            #[allow(dead_code)]
            #[allow(clippy::manual_c_str_literals)]
            /// # Safety
            /// Interoperability with C code.
            /// Ensure that the pointers are valid.
            pub unsafe fn subscribe(
                pubsub: *mut freerdp_sys::wPubSub,
                handler: $handler_ty,
            ) -> i32 {
                unsafe {
                    freerdp_sys::PubSub_Subscribe(
                        pubsub,
                        concat!(stringify!($name), "\0").as_ptr() as *const i8,
                        handler,
                    )
                }
            }

            #[allow(dead_code)]
            #[allow(clippy::manual_c_str_literals)]
            /// # Safety
            /// Interoperability with C code.
            /// Ensure that the pointers are valid.
            pub unsafe fn unsubscribe(
                pubsub: *mut freerdp_sys::wPubSub,
                handler: $handler_ty,
            ) -> i32 {
                unsafe {
                    freerdp_sys::PubSub_Unsubscribe(
                        pubsub,
                        concat!(stringify!($name), "\0").as_ptr() as *const i8,
                        handler,
                    )
                }
            }
        }
    };
}

define_event!(
    ChannelConnected,
    freerdp_sys::ChannelConnectedEventArgs,
    freerdp_sys::pChannelConnectedEventHandler
);

define_event!(
    ChannelDisconnected,
    freerdp_sys::ChannelDisconnectedEventArgs,
    freerdp_sys::pChannelDisconnectedEventHandler
);

define_event!(
    ChannelAttached,
    freerdp_sys::ChannelAttachedEventArgs,
    freerdp_sys::pChannelAttachedEventHandler
);

define_event!(
    ChannelDetached,
    freerdp_sys::ChannelDetachedEventArgs,
    freerdp_sys::pChannelDetachedEventHandler
);
