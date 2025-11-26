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
