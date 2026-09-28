/*
 * librivet from C (PROP-2026-0002 UC-06, R13–R15): a blocking request, a
 * stream, live input and cancellation, plus highlighting.
 *
 *   rivet_runtime_new ─▶ rivet_request ─▶ envelope
 *   rivet_call_start  ─▶ rivet_call_next … NULL          (events.count: data, data, data, result)
 *                     ─▶ rivet_call_send / finish_input  (chat.echo: live input)
 *                     ─▶ rivet_call_cancel               (result status "cancelled")
 *
 *   make -C examples/c run          (shared librivet; see the Makefile for static)
 *   ./demo [BUNDLE]                 (default ../ffi/app.rivet)
 */
#include <stdio.h>
#include <string.h>
#include <rivet.h>

/* Print and free one returned string. */
static void show(const char *label, char *s) {
    printf("%s %s\n", label, s ? s : "(null)");
    rivet_string_free(s);
}

/* Drain a call until its terminal record; returns the number of records. */
static int drain(RivetCall *call) {
    int n = 0;
    char *rec;
    while ((rec = rivet_call_next(call, 5000)) != NULL) {
        if (strcmp(rec, "{\"type\":\"timeout\"}") != 0) {
            n++;
        }
        show("record", rec);
    }
    return n;
}

int main(int argc, char **argv) {
    const char *bundle = argc > 1 ? argv[1] : "../ffi/app.rivet";
    char options[1024];
    char *err = NULL;
    RivetRuntime *rt = NULL;

    printf("abi %u version %s\n", rivet_abi_version(), rivet_version());

    /* A bad option is an error envelope, never a crash. */
    if (rivet_runtime_new("{\"file\":\"x.rivet\",\"colour\":true}", &rt, &err) != RIVET_OK) {
        show("options-error", err);
    }

    snprintf(options, sizeof options, "{\"file\":\"%s\"}", bundle);
    if (rivet_runtime_new(options, &rt, &err) != RIVET_OK) {
        show("runtime-error", err);
        return 1;
    }

    /* Blocking request; "pretty": true indents this one envelope. */
    show("request", rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":2,\"b\":3}}"));
    show("pretty", rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":40,\"b\":2},\"pretty\":true}"));
    show("invalid", rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":\"two\"}}"));

    /* A server stream: data records, then one result record, then NULL. */
    RivetCall *stream = rivet_call_start(rt, "{\"operation\":\"events.count\",\"data\":{}}");
    printf("stream-records %d\n", drain(stream));
    rivet_call_free(stream);

    /* Live input: send two items, close the input, read everything. */
    RivetCall *chat = rivet_call_start(rt, "{\"operation\":\"chat.echo\",\"data\":{}}");
    show("send", rivet_call_send(chat, "\"hi\""));
    show("send", rivet_call_send(chat, "\"there\""));
    show("finish", rivet_call_finish_input(chat));
    printf("chat-records %d\n", drain(chat));
    rivet_call_free(chat);

    /* Cancellation: the echo waits for input until it is cancelled. */
    RivetCall *waiting = rivet_call_start(rt, "{\"operation\":\"chat.echo\",\"data\":{}}");
    show("send", rivet_call_send(waiting, "\"ping\""));
    show("first", rivet_call_next(waiting, 5000));
    show("poll", rivet_call_next(waiting, 50)); /* nothing new: {"type":"timeout"} */
    rivet_call_cancel(waiting);
    printf("cancel-records %d\n", drain(waiting));
    rivet_call_free(waiting);

    /* Highlighting needs no runtime. */
    char *tokens = rivet_highlight("global api = \"https://api.example.com\"\n", "json");
    printf("highlight %.*s\n", (int)strcspn(tokens, "\n"), tokens);
    rivet_string_free(tokens);

    /* Double free is refused, not undefined behaviour. */
    printf("free %d\n", rivet_runtime_free(rt));
    printf("free-again %d\n", rivet_runtime_free(rt));
    return 0;
}
