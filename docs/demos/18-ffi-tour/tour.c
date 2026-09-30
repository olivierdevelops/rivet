/*
 * tour.c — every librivet capability in one program (DEMO-2026-0021).
 *
 * Built outside the repository against the shipped dist/v0.2.1 files:
 *   cc tour.c $(pkg-config --cflags --libs rivet) -lpthread -o tour
 * Run from this folder (app.rivet, policy.json, data/, lib/ beside it).
 */
#include <pthread.h>
#include <stdio.h>
#include <string.h>

#include "rivet.h"

/* Print a returned string under a label and free it. */
static void show(const char *label, char *s) {
    printf("%-14s %s\n", label, s ? s : "(NULL)");
    rivet_string_free(s);
}

/* Drain a call: print every record until NULL, then free the call. */
static void drain(const char *label, RivetCall *call) {
    char *rec;
    while ((rec = rivet_call_next(call, 2000)) != NULL) show(label, rec);
    rivet_call_free(call);
}

#define THREADS 8
#define PER_THREAD 250
static RivetRuntime *shared_rt;

static void *worker(void *arg) {
    long ok = 0, n = (long)arg;
    char input[128];
    for (int i = 0; i < PER_THREAD; i++) {
        snprintf(input, sizeof input,
                 "{\"operation\":\"demo.add\",\"data\":{\"a\":%ld,\"b\":%d}}", n, i);
        char *out = rivet_request(shared_rt, input);
        char want[48];
        snprintf(want, sizeof want, "\"data\":%ld,", n + i);
        if (strstr(out, "\"status\":\"ok\"") && strstr(out, want)) ok++;
        rivet_string_free(out);
    }
    return (void *)ok;
}

int main(void) {
    char *err = NULL;
    RivetRuntime *rt = NULL;

    puts("== 1. identity");
    printf("%-14s abi %u, version %s\n", "library", rivet_abi_version(), rivet_version());

    puts("== 2. runtime: bad options are an envelope, not a crash");
    if (rivet_runtime_new("{\"file\":\"app.rivet\",\"colour\":true}", &rt, &err) != RIVET_OK)
        show("options-error", err);
    if (rivet_runtime_new("{\"file\":\"app.rivet\",\"policy_file\":\"policy.json\"}", &rt, &err) != RIVET_OK) {
        show("fatal", err);
        return 1;
    }

    puts("== 3. unary requests, pretty output, validation, globals");
    show("add", rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":2,\"b\":3}}"));
    show("pretty", rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":40,\"b\":2},\"pretty\":true}"));
    show("invalid", rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":\"two\"}}"));
    show("unknown", rivet_request(rt, "{\"operation\":\"demo.nope\",\"data\":{}}"));
    show("global", rivet_request(rt, "{\"operation\":\"demo.greet\",\"data\":{\"who\":\"C\"}}"));

    puts("== 4. the sandbox policy applies through the ABI");
    show("allowed", rivet_request(rt, "{\"operation\":\"data.read\",\"data\":{}}"));
    show("denied", rivet_request(rt, "{\"operation\":\"data.private\",\"data\":{}}"));
    show("restricted", rivet_request(rt,
        "{\"operation\":\"data.read\",\"data\":{},\"restrict\":{\"grants\":["
        "{\"capability\":\"allow_read\",\"targets\":[\"./data/other/**\"],\"access\":[\"read\"]}]}}"));

    puts("== 5. server stream");
    drain("stream", rivet_call_start(rt, "{\"operation\":\"events.count\",\"data\":{}}"));

    puts("== 6. live input: send, timeout record, finish");
    RivetCall *chat = rivet_call_start(rt, "{\"operation\":\"chat.echo\",\"data\":{}}");
    show("send", rivet_call_send(chat, "\"hi\""));
    show("next", rivet_call_next(chat, 2000));
    show("next(100ms)", rivet_call_next(chat, 100));
    show("send", rivet_call_send(chat, "\"bye\""));
    show("finish", rivet_call_finish_input(chat));
    drain("next", chat);

    puts("== 7. cancel and deadline");
    RivetCall *c2 = rivet_call_start(rt, "{\"operation\":\"chat.echo\",\"data\":{}}");
    rivet_call_cancel(c2);
    drain("cancelled", c2);
    drain("deadline", rivet_call_start(rt, "{\"operation\":\"chat.echo\",\"data\":{},\"deadline_ms\":200}"));

    puts("== 8. modules: a file as an object of operations");
    RivetModule *users = NULL;
    if (rivet_load(rt, "lib/users.rivet", "users", &users, &err) != RIVET_OK) {
        show("load-error", err);
        return 1;
    }
    show("operations", rivet_module_operations(users));
    show("users.get", rivet_module_call(users, "get", "{\"id\":7}"));
    drain("users.list", rivet_module_call_start(users, "list", NULL));
    RivetModule *outside = NULL;
    if (rivet_load(rt, "../15-ffi/app.rivet", "x", &outside, &err) != RIVET_OK)
        show("outside-root", err);
    rivet_module_free(users);

    puts("== 9. one runtime, many threads");
    shared_rt = rt;
    pthread_t t[THREADS];
    long total = 0;
    for (long i = 0; i < THREADS; i++) pthread_create(&t[i], NULL, worker, (void *)i);
    for (int i = 0; i < THREADS; i++) {
        void *ok;
        pthread_join(t[i], &ok);
        total += (long)ok;
    }
    printf("%-14s %ld/%d correct envelopes from %d threads\n", "threads", total,
           THREADS * PER_THREAD, THREADS);

    puts("== 10. highlighting (no runtime needed)");
    show("json", rivet_highlight("global N = 1", "json"));
    show("html", rivet_highlight("global N = 1", "html"));

    puts("== 11. misuse is refused, not undefined behaviour");
    show("null-input", rivet_request(rt, NULL));
    char local[] = "not from librivet";
    printf("%-14s %s\n", "foreign-free", rivet_string_free(local) == RIVET_ERROR ? "RIVET_ERROR" : "RIVET_OK");
    printf("%-14s %s\n", "runtime-free", rivet_runtime_free(rt) == RIVET_OK ? "RIVET_OK" : "RIVET_ERROR");
    printf("%-14s %s\n", "double-free", rivet_runtime_free(rt) == RIVET_ERROR ? "RIVET_ERROR" : "RIVET_OK");
    show("freed-handle", rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":1}}"));
    return 0;
}
