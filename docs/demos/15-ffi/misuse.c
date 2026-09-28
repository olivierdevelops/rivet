/*
 * librivet misuse is an error envelope, never a crash (PROP-2026-0002 R14).
 *
 *   NULL handle / NULL string   ─▶ validation.ffi_argument
 *   invalid UTF-8               ─▶ validation.ffi_argument
 *   malformed JSON input        ─▶ validation.input_envelope
 *   freeing a runtime twice     ─▶ RIVET_ERROR (1), not undefined behaviour
 *
 *   cc misuse.c -I ../../../ffi/include -L ../../../target/release -lrivet \
 *      -Wl,-rpath,"$PWD/../../../target/release" -o misuse && ./misuse
 */
#include <stdio.h>
#include <rivet.h>

static void show(const char *label, char *s) {
    printf("%s %s\n", label, s ? s : "(null)");
    rivet_string_free(s);
}

int main(void) {
    RivetRuntime *rt = NULL;
    char *err = NULL;
    if (rivet_runtime_new("{\"file\":\"app.rivet\"}", &rt, &err) != RIVET_OK) {
        show("runtime-error", err);
        return 1;
    }
    show("null-runtime", rivet_request(NULL, "{\"operation\":\"demo.add\",\"data\":{\"a\":1}}"));
    show("null-input", rivet_request(rt, NULL));
    show("bad-utf8", rivet_request(rt, "{\"operation\":\"demo.\xff\"}"));
    show("bad-json", rivet_request(rt, "{\"operation\":"));
    RivetRuntime *other = NULL;
    if (rivet_runtime_new("{\"file\":\"nope.rivet\"}", &other, &err) != RIVET_OK) {
        show("missing-file", err);
    }
    printf("free %d\n", rivet_runtime_free(rt));
    printf("free-again %d\n", rivet_runtime_free(rt));
    return 0;
}
