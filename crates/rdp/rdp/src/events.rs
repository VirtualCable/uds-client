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
// Defines the event handling mechanism for RDP events using FreeRDP's PubSub system.
#[macro_export]
macro_rules! define_event {
    ($name:ident, $args_ty:ty, $handler_ty:ty) => {
        pub struct $name;

        impl $name {
            #[allow(dead_code)]
            #[allow(clippy::manual_c_str_literals)]
            #[allow(clippy::not_unsafe_ptr_arg_deref)]
            pub fn fire_event(
                pubsub: *mut freerdp_sys::wPubSub,
                context: *mut std::ffi::c_void,
                e: *const $args_ty,
            ) -> i32 {
                unsafe {
                    freerdp_sys::PubSub_OnEvent(
                        pubsub,
                        concat!(stringify!($name), "\0").as_ptr() as *const ::std::os::raw::c_char,
                        context,
                        &(*e).e,
                    )
                }
            }

            #[allow(dead_code)]
            #[allow(clippy::manual_c_str_literals)]
            #[allow(clippy::not_unsafe_ptr_arg_deref)]
            pub fn subscribe(pubsub: *mut freerdp_sys::wPubSub, handler: $handler_ty) -> i32 {
                unsafe {
                    freerdp_sys::PubSub_Subscribe(
                        pubsub,
                        concat!(stringify!($name), "\0").as_ptr() as *const ::std::os::raw::c_char,
                        handler,
                    )
                }
            }

            #[allow(dead_code)]
            #[allow(clippy::manual_c_str_literals)]
            #[allow(clippy::not_unsafe_ptr_arg_deref)]
            pub fn unsubscribe(pubsub: *mut freerdp_sys::wPubSub, handler: $handler_ty) -> i32 {
                unsafe {
                    freerdp_sys::PubSub_Unsubscribe(
                        pubsub,
                        concat!(stringify!($name), "\0").as_ptr() as *const ::std::os::raw::c_char,
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
