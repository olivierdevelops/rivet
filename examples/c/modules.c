/*
 * Load .rivet files as module objects from C (PROP-2026-0002 UC-11, R23).
 *
 *   rivet_runtime_new({"root": DIR})        a runtime with no entry file
 *   rivet_load(rt, "./users.rivet", NULL)   module "users" (alias = file stem)
 *   rivet_module_operations(users)          [{"id":"get",…},{"id":"list",…}]
 *   rivet_module_call(users, "get", …)      envelope, operation "users.get"
 *
 *   make -C examples/c run-modules          ./modules [DIR] (default ../modules)
 */
#include <stdio.h>
#include <rivet.h>

static void show(const char *label, char *s) {
    printf("%s %s\n", label, s ? s : "(null)");
    rivet_string_free(s);
}

int main(int argc, char **argv) {
    const char *root = argc > 1 ? argv[1] : "../modules";
    char options[1024];
    char *err = NULL;
    RivetRuntime *rt = NULL;
    snprintf(options, sizeof options, "{\"root\":\"%s\"}", root);
    if (rivet_runtime_new(options, &rt, &err) != RIVET_OK) {
        show("runtime-error", err);
        return 1;
    }

    RivetModule *users = NULL;
    if (rivet_load(rt, "./users.rivet", NULL, &users, &err) != RIVET_OK) {
        show("load-error", err);
        return 1;
    }
    show("operations", rivet_module_operations(users));
    show("call", rivet_module_call(users, "get", "{\"id\":42}"));
    show("bad-call", rivet_module_call(users, "get", "{\"id\":0}"));

    /* A module that imports another; loaded under an explicit alias. */
    RivetModule *billing = NULL;
    if (rivet_load(rt, "./lib/billing.rivet", "billing", &billing, &err) != RIVET_OK) {
        show("load-error", err);
        return 1;
    }
    show("call", rivet_module_call(billing, "invoice", "{\"user\":7}"));

    /* The same alias twice is refused with an error envelope. */
    RivetModule *again = NULL;
    if (rivet_load(rt, "./users.rivet", NULL, &again, &err) != RIVET_OK) {
        show("duplicate", err);
    }

    /* A call handle on a module operation. */
    RivetCall *call = rivet_module_call_start(users, "list", NULL);
    char *rec;
    while ((rec = rivet_call_next(call, 5000)) != NULL) {
        show("record", rec);
    }
    rivet_call_free(call);

    /* The runtime's own dispatcher sees the namespaced IDs too. */
    show("request", rivet_request(rt, "{\"operation\":\"users.get\",\"data\":{\"id\":7}}"));

    rivet_module_free(billing);
    rivet_module_free(users);
    printf("module-free-again %d\n", rivet_module_free(users));
    rivet_runtime_free(rt);
    return 0;
}
