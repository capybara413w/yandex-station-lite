#include <stdio.h>
#include <stdlib.h>
#include <dlfcn.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
#include <signal.h>
#include <sys/stat.h>
#include <dirent.h>

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

static void find_model_path(char* out_path, size_t max_len, const char* keyword, const char* default_path) {
    DIR* d = opendir("/data/quasar/data/spotter_model");
    if (d) {
        struct dirent* dir;
        while ((dir = readdir(d)) != NULL) {
            if (dir->d_name[0] == '.') continue;
            char candidate[256];
            snprintf(candidate, sizeof(candidate), "/data/quasar/data/spotter_model/%s", dir->d_name);
            
            if (strcmp(keyword, "ca") == 0) {
                char check[300];
                snprintf(check, sizeof(check), "%s/counteractivation-features", candidate);
                if (file_exists(check)) {
                    strncpy(out_path, candidate, max_len - 1);
                    out_path[max_len - 1] = '\0';
                    closedir(d);
                    return;
                }
            } else {
                char words_file[300];
                snprintf(words_file, sizeof(words_file), "%s/words.txt", candidate);
                FILE* wf = fopen(words_file, "r");
                if (wf) {
                    char line[128];
                    while (fgets(line, sizeof(line), wf)) {
                        if (strstr(line, keyword)) {
                            fclose(wf);
                            strncpy(out_path, candidate, max_len - 1);
                            out_path[max_len - 1] = '\0';
                            closedir(d);
                            return;
                        }
                    }
                    fclose(wf);
                }
            }
        }
        closedir(d);
    }
    strncpy(out_path, default_path, max_len - 1);
    out_path[max_len - 1] = '\0';
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

    char alisa_path[256];
    char vad_path[256];
    char ca_path[256];

    find_model_path(alisa_path, sizeof(alisa_path), "алиса", "/system/vendor/quasar/spotter_models/activation/alisa");
    find_model_path(vad_path, sizeof(vad_path), "<speech_started>", "/system/vendor/quasar/spotter_models/vad");
    find_model_path(ca_path, sizeof(ca_path), "ca", "/system/vendor/quasar/spotter_models/counteractivation");

    if (verbose) {
        fprintf(stdout, "[DEBUG] Alisa model: %s\n", alisa_path);
        fprintf(stdout, "[DEBUG] VAD model: %s\n", vad_path);
        fprintf(stdout, "[DEBUG] CA model: %s\n", ca_path);
    }

    void* err = NULL;
    void* conf_alisa = conf_read(alisa_path, &err);
    void* conf_vad = conf_read(vad_path, &err);
    void* conf_ca = conf_read(ca_path, &err);

    if (!conf_alisa || !conf_vad || !conf_ca) {
        fprintf(stdout, "ERROR:model_load_failed\n");
        return 1;
    }

    void* multi_conf = multi_create("Y", 1, &err);
    if (!multi_conf) {
        fprintf(stdout, "ERROR:multi_create_failed\n");
        return 1;
    }

    multi_attach(multi_conf, conf_alisa, &err);
    multi_attach(multi_conf, conf_vad, &err);
    multi_attach(multi_conf, conf_ca, &err);

    int channels[3] = {0, 3, 4};
    size_t num_components = 3;

    void* spotter = create_multi(multi_conf, 16000, num_components, channels, 1, 0, &err);
    if (!spotter) {
        fprintf(stdout, "ERROR:spotter_create_failed\n");
        return 1;
    }

    fprintf(stdout, "READY\n");
    fflush(stdout);

    // Stream audio processing:
    // Takes 800 samples of 16-bit mono PCM (50ms chunks = 1600 bytes)
    // Duplicates mono sample across the 3 active components [s, s, s]
    int16_t mono_buf[800];
    int16_t tri_buf[800 * 3];

    while (running) {
        size_t samples_read = fread(mono_buf, sizeof(int16_t), 800, stdin);
        if (samples_read == 0) {
            break;
        }

        for (size_t i = 0; i < samples_read; i++) {
            int16_t s = mono_buf[i];
            tri_buf[i * 3 + 0] = s;
            tri_buf[i * 3 + 1] = s;
            tri_buf[i * 3 + 2] = s;
        }

        size_t bytes_to_submit = samples_read * 3 * sizeof(int16_t);
        int ret = submit_raw(spotter, (const char*)tri_buf, bytes_to_submit, &err);
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
