#include <stdio.h>
#include <stdlib.h>
#include <dlfcn.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
#include <signal.h>
#include <sys/stat.h>

typedef void* (*spotter_conf_read_fn)(const char* dir_path, void** err);
typedef void* (*spotter_multi_conf_create_fn)(const char* name, size_t name_len, void** err);
typedef int (*spotter_multi_conf_attach_spotter_conf_fn)(void* multi_conf, void* spotter_conf, void** err);
typedef void* (*spotter_create_from_multi_conf_fn)(
    void* multi_conf,
    int sample_rate,
    size_t num_components,
    const int* component_channels,
    int extra1,
    int extra2,
    void** err
);
typedef void (*spotter_destroy_fn)(void* spotter);
typedef const char* (*spotter_error_get_message_fn)(void* err);
typedef int (*spotter_submit_raw_data_fn)(void* spotter, const char* data, size_t size, void** err);
typedef int (*spotter_get_next_event_fn)(void* spotter, void** err);
typedef const char* (*event_id_to_str_fn)(int event_id);
typedef const char* (*spotter_get_current_phrase_fn)(void* spotter);

static volatile int running = 1;
static void handle_sig(int sig) {
    (void)sig;
    running = 0;
}

static int file_exists(const char* path) {
    struct stat st;
    return (stat(path, &st) == 0);
}

int main(int argc, char** argv) {
    setvbuf(stdout, NULL, _IONBF, 0);

    int verbose = 0;
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-v") == 0 || strcmp(argv[i], "--verbose") == 0) {
            verbose = 1;
        }
    }

    if (!verbose) {
        // Suppress noisy internal C++ logs from libyandex_evil to stderr
        freopen("/dev/null", "w", stderr);
    }

    signal(SIGTERM, handle_sig);
    signal(SIGINT, handle_sig);
    signal(SIGPIPE, handle_sig);

    void* handle = dlopen("/system/vendor/quasar/libs/libyandex_evil.so", RTLD_NOW);
    if (!handle) {
        fprintf(stdout, "ERROR:dlopen_failed\n");
        return 1;
    }

    spotter_conf_read_fn conf_read = (spotter_conf_read_fn)dlsym(handle, "spotter_conf_read");
    spotter_multi_conf_create_fn multi_create = (spotter_multi_conf_create_fn)dlsym(handle, "spotter_multi_conf_create");
    spotter_multi_conf_attach_spotter_conf_fn multi_attach = (spotter_multi_conf_attach_spotter_conf_fn)dlsym(handle, "spotter_multi_conf_attach_spotter_conf");
    spotter_create_from_multi_conf_fn create_multi = (spotter_create_from_multi_conf_fn)dlsym(handle, "spotter_create_from_multi_conf");
    spotter_destroy_fn spotter_destroy = (spotter_destroy_fn)dlsym(handle, "spotter_destroy");
    spotter_error_get_message_fn err_msg = (spotter_error_get_message_fn)dlsym(handle, "spotter_error_get_message");
    spotter_submit_raw_data_fn submit_raw = (spotter_submit_raw_data_fn)dlsym(handle, "spotter_submit_raw_data");
    spotter_get_next_event_fn get_event = (spotter_get_next_event_fn)dlsym(handle, "spotter_get_next_event");
    spotter_get_current_phrase_fn get_phrase = (spotter_get_current_phrase_fn)dlsym(handle, "spotter_get_current_phrase");

    if (!conf_read || !multi_create || !multi_attach || !create_multi || !submit_raw || !get_event) {
        fprintf(stdout, "ERROR:dlsym_failed\n");
        return 1;
    }

    // Active model directories on Station Lite
    const char* alisa_path = "/data/quasar/data/spotter_model/4b4cbc57-338c-499c-a2d0-2edf81e00aa2";
    const char* vad_path   = "/data/quasar/data/spotter_model/386347ed-af5f-4b35-8c70-d4bb88be1c9b";
    const char* ca_path    = "/data/quasar/data/spotter_model/b7b728b2-fd2f-4d89-999b-eb2fdb44ed3b";

    // Fallbacks if data models were reset or missing
    if (!file_exists(alisa_path)) {
        alisa_path = "/system/vendor/quasar/spotter_models/activation/alisa";
    }

    void* err = NULL;
    void* conf_alisa = conf_read(alisa_path, &err);
    void* conf_vad = file_exists(vad_path) ? conf_read(vad_path, &err) : NULL;
    void* conf_ca  = file_exists(ca_path)  ? conf_read(ca_path, &err)  : NULL;

    if (!conf_alisa) {
        fprintf(stdout, "ERROR:model_load_failed\n");
        return 1;
    }

    void* multi_conf = multi_create("Y", 1, &err);
    if (!multi_conf) {
        fprintf(stdout, "ERROR:multi_create_failed\n");
        return 1;
    }

    multi_attach(multi_conf, conf_alisa, &err);
    size_t num_components = 1;
    int channels[3] = {0, 0, 0};

    if (conf_vad) {
        multi_attach(multi_conf, conf_vad, &err);
        channels[num_components++] = 3;
    }
    if (conf_ca) {
        multi_attach(multi_conf, conf_ca, &err);
        channels[num_components++] = 4;
    }

    void* spotter = create_multi(multi_conf, 16000, num_components, channels, 1, 0, &err);
    if (!spotter) {
        fprintf(stdout, "ERROR:spotter_create_failed\n");
        return 1;
    }

    fprintf(stdout, "READY\n");
    fflush(stdout);

    // Audio stream processing:
    // Takes 800 samples of 16-bit mono PCM (50ms chunks = 1600 bytes)
    // Duplicates mono sample across active components
    int16_t mono_buf[800];
    int16_t multi_buf[800 * 3];

    while (running) {
        size_t samples_read = fread(mono_buf, sizeof(int16_t), 800, stdin);
        if (samples_read == 0) {
            break;
        }

        for (size_t i = 0; i < samples_read; i++) {
            int16_t s = mono_buf[i];
            for (size_t c = 0; c < num_components; c++) {
                multi_buf[i * num_components + c] = s;
            }
        }

        size_t bytes_to_submit = samples_read * num_components * sizeof(int16_t);
        int ret = submit_raw(spotter, (const char*)multi_buf, bytes_to_submit, &err);
        if (ret < 0) {
            break;
        }

        while (running) {
            int ev = get_event(spotter, &err);
            if (ev <= 0) break; // 0 = EVENT_NEED_MORE_DATA_FOR_1ST_STAGE

            // EVENT 2 = EVENT_CONFIRMATION_1ST_STAGE
            // EVENT 5 = EVENT_CONFIRMATION_2ND_STAGE
            if (ev == 2 || ev == 5) {
                const char* phrase = get_phrase ? get_phrase(spotter) : NULL;
                if (phrase && (strcmp(phrase, "алиса") == 0 || strcmp(phrase, "яндекс") == 0)) {
                    fprintf(stdout, "WAKEWORD:%s\n", phrase);
                    fflush(stdout);
                }
            }
        }
    }

    if (spotter_destroy) spotter_destroy(spotter);
    dlclose(handle);
    return 0;
}
