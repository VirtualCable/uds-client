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
#include <freerdp/api.h>
#include <freerdp/freerdp.h>
#include <freerdp/types.h>
#include <stdarg.h>

// Callback type expected from Rust side.
// Note: data is an array of const char* (strings).
typedef BOOL (*pGetAccessTokenNoVarargs)(freerdp *instance,
                                         AccessTokenType tokenType,
                                         char **token,
                                         size_t count,
                                         const char **data);

// Function pointer to hold the Rust callback
static pGetAccessTokenNoVarargs rust_get_access_token_cb = NULL;

// Setter for registering the Rust callback
void set_rust_get_access_token_cb(pGetAccessTokenNoVarargs cb) {
    rust_get_access_token_cb = cb;
}

// Wrapper: collects varargs into an array of const char* and forwards to Rust callback
BOOL get_access_token_wrapper(freerdp *instance, AccessTokenType tokenType,
                              char **token, size_t count, ...) {
    const char **data = NULL;
    BOOL result = FALSE;

    if (count > 0) {
        va_list args;
        va_start(args, count);

        // Allocate array of const char* (strings)
        data = malloc(count * sizeof(const char *));
        if (!data) {
            va_end(args);
            return FALSE; // allocation failed
        }

        for (size_t i = 0; i < count; i++) {
            // Each argument is expected to be a const char*
            data[i] = va_arg(args, const char *);
        }

        va_end(args);
    }

    if (rust_get_access_token_cb) {
        // Forward collected arguments to Rust callback
        result = rust_get_access_token_cb(instance, tokenType, token, count, data);
    }

    free(data);
    return result;
}
