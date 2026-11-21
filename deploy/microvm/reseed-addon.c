#define _GNU_SOURCE
#include <dlfcn.h>
#include <node_api.h>

/* Resolve symbols in the Node executable. This addon links no libcrypto. */
typedef struct evp_rand_ctx_st Rand;
typedef struct ossl_lib_ctx_st Lib;
typedef Rand *(*get_rng)(Lib *);
typedef int (*reseed_rng)(Rand *, int, const unsigned char *, size_t,
                         const unsigned char *, size_t);

static napi_value reseed(napi_env env, napi_callback_info info) {
    (void)info;
    reseed_rng force = dlsym(RTLD_DEFAULT, "EVP_RAND_reseed");
    const char *names[] = {"RAND_get0_primary", "RAND_get0_public", "RAND_get0_private"};
    if (!force) {
        napi_throw_error(env, NULL, "Node does not export EVP_RAND_reseed");
        return NULL;
    }
    Dl_info origin = {0};
    if (!dladdr((void *)force, &origin) || !origin.dli_fname) {
        napi_throw_error(env, NULL, "Cannot identify EVP_RAND_reseed provider");
        return NULL;
    }
    /* The launcher is idle, with no outstanding crypto work. Public and
     * private here are this thread's contexts. All calls request live entropy. */
    for (size_t i = 0; i < sizeof names / sizeof names[0]; i++) {
        get_rng getter = dlsym(RTLD_DEFAULT, names[i]);
        Rand *rng = getter ? getter(NULL) : NULL;
        if (!rng || force(rng, 1, NULL, 0, NULL, 0) != 1) {
            napi_throw_error(env, NULL, names[i]);
            return NULL;
        }
    }
    napi_value result;
    if (napi_create_string_utf8(env, origin.dli_fname, NAPI_AUTO_LENGTH, &result) != napi_ok)
        return NULL;
    return result;
}

NAPI_MODULE_INIT() {
    napi_value method;
    if (napi_create_function(env, "reseed", NAPI_AUTO_LENGTH, reseed, NULL, &method) != napi_ok)
        return NULL;
    if (napi_set_named_property(env, exports, "reseed", method) != napi_ok) return NULL;
    return exports;
}
