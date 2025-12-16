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
pub fn interpolate(raw: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut result = raw.to_string();
    for arg in args {
        if result.contains("{}") {
            result = result.replacen("{}", &arg.to_string(), 1);
        }
    }
    result
}

#[macro_export]
macro_rules! tr {
    // Simple translation
    ($msg:expr) => {
        $crate::intl::CATALOG.gettext($msg)
    };

    // Translation with parameters
    ($msg:expr, $($arg:expr),+ $(,)?) => {{
        let raw = $crate::intl::CATALOG.gettext($msg);
        $crate::intl::macros::interpolate(&raw, &[ $( &$arg ),+ ])
    }};

    // Plural translation
    ($sing:expr, $plur:expr, $n:expr) => {{
        let raw = $crate::intl::CATALOG.ngettext($sing, $plur, $n);
        $crate::intl::macros::interpolate(&raw, &[ &$n ])
    }};
}
