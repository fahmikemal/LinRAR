#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <errno.h>

#define SFX_FOOTER_MAGIC 0x444E45584653524CULL // "LRSFXEND" in Little-Endian
#define SFX_HEADER_MAGIC "LINRSFX1"

#define SFX_FLAG_OVERWRITE (1 << 0)
#define SFX_FLAG_QUIET     (1 << 1)

#define SFX_TYPE_TARGZ 1
#define SFX_TYPE_ZIP   2

typedef struct __attribute__((packed)) {
    char     magic[8];        // "LINRSFX1"
    uint32_t header_size;     // Total size of header including string payload
    uint32_t payload_type;    // 1 = tar.gz, 2 = zip
    uint32_t flags;           // Default flags
    uint64_t payload_len;     // Exact byte length of payload
    uint32_t title_len;       // Length of title string
    uint32_t target_len;      // Length of default target dir string
    uint32_t run_len;         // Length of post-extract run command
} SfxHeader;

static void print_banner(const char *title) {
    printf("===================================================================\n");
    printf(" LinRAR Self-Extracting Archive (Linux ELF SFX)\n");
    if (title && title[0]) {
        printf(" Archive: %s\n", title);
    }
    printf("===================================================================\n");
}

static void print_help(const char *prog, const char *title, const char *def_target) {
    print_banner(title);
    printf("Usage: %s [OPTIONS]\n\n", prog);
    printf("Options:\n");
    printf("  -d, --target <DIR>    Extract files to target directory [default: '%s']\n", def_target ? def_target : ".");
    printf("  -l, --list            List files in this archive without extracting\n");
    printf("  -t, --test            Verify archive integrity and test extraction\n");
    printf("  -y, --overwrite       Overwrite existing files without prompting\n");
    printf("  -q, --quiet           Silent mode (suppress extraction messages)\n");
    printf("  -h, --help            Show this help information\n\n");
}

static int stream_payload_to_cmd(FILE *self_file, uint64_t payload_offset, uint64_t payload_len, const char *cmd) {
    FILE *proc = popen(cmd, "w");
    if (!proc) {
        fprintf(stderr, "Error: Failed to spawn extraction process '%s': %s\n", cmd, strerror(errno));
        return 1;
    }

    if (fseek(self_file, (long)payload_offset, SEEK_SET) != 0) {
        fprintf(stderr, "Error: Failed to seek to SFX payload offset: %s\n", strerror(errno));
        pclose(proc);
        return 1;
    }

    char buffer[65536];
    uint64_t remaining = payload_len;
    while (remaining > 0) {
        size_t to_read = (remaining > sizeof(buffer)) ? sizeof(buffer) : (size_t)remaining;
        size_t n = fread(buffer, 1, to_read, self_file);
        if (n == 0) {
            if (feof(self_file)) {
                fprintf(stderr, "Error: Unexpected end of SFX file while reading payload.\n");
            } else {
                fprintf(stderr, "Error reading SFX payload: %s\n", strerror(errno));
            }
            pclose(proc);
            return 1;
        }
        size_t written = fwrite(buffer, 1, n, proc);
        if (written != n) {
            fprintf(stderr, "Error writing to extraction process.\n");
            pclose(proc);
            return 1;
        }
        remaining -= n;
    }

    int status = pclose(proc);
    if (WIFEXITED(status)) {
        return WEXITSTATUS(status);
    }
    return 1;
}

int main(int argc, char **argv) {
    char exe_path[4096];
    ssize_t link_len = readlink("/proc/self/exe", exe_path, sizeof(exe_path) - 1);
    if (link_len <= 0) {
        strncpy(exe_path, argv[0], sizeof(exe_path) - 1);
        exe_path[sizeof(exe_path) - 1] = '\0';
    } else {
        exe_path[link_len] = '\0';
    }

    FILE *f = fopen(exe_path, "rb");
    if (!f) {
        fprintf(stderr, "Error: Cannot open self executable '%s': %s\n", exe_path, strerror(errno));
        return 1;
    }

    // Read footer at EOF - 16
    if (fseek(f, -16, SEEK_END) != 0) {
        fprintf(stderr, "Error: Corrupted SFX file: unable to seek to footer.\n");
        fclose(f);
        return 1;
    }

    uint64_t footer_magic = 0, header_offset = 0;
    if (fread(&footer_magic, 8, 1, f) != 1 || fread(&header_offset, 8, 1, f) != 1) {
        fprintf(stderr, "Error: Failed to read SFX footer.\n");
        fclose(f);
        return 1;
    }

    if (footer_magic != SFX_FOOTER_MAGIC) {
        fprintf(stderr, "Error: Invalid SFX footer magic. This binary is not an intact LinRAR SFX archive.\n");
        fclose(f);
        return 1;
    }

    // Seek to header
    if (fseek(f, (long)header_offset, SEEK_SET) != 0) {
        fprintf(stderr, "Error: Failed to seek to SFX header at offset %lu.\n", (unsigned long)header_offset);
        fclose(f);
        return 1;
    }

    SfxHeader hdr;
    if (fread(&hdr, sizeof(hdr), 1, f) != 1) {
        fprintf(stderr, "Error: Failed to read SFX header.\n");
        fclose(f);
        return 1;
    }

    if (memcmp(hdr.magic, SFX_HEADER_MAGIC, 8) != 0) {
        fprintf(stderr, "Error: Corrupted SFX header magic.\n");
        fclose(f);
        return 1;
    }

    char title[512] = {0};
    char default_target[1024] = {0};
    char run_after[1024] = {0};

    if (hdr.title_len > 0 && hdr.title_len < sizeof(title)) {
        if (fread(title, 1, hdr.title_len, f) != hdr.title_len) {
            fprintf(stderr, "Error reading SFX title.\n");
            fclose(f);
            return 1;
        }
        title[hdr.title_len] = '\0';
    }

    if (hdr.target_len > 0 && hdr.target_len < sizeof(default_target)) {
        if (fread(default_target, 1, hdr.target_len, f) != hdr.target_len) {
            fprintf(stderr, "Error reading default target directory.\n");
            fclose(f);
            return 1;
        }
        default_target[hdr.target_len] = '\0';
    } else {
        strcpy(default_target, ".");
    }

    if (hdr.run_len > 0 && hdr.run_len < sizeof(run_after)) {
        if (fread(run_after, 1, hdr.run_len, f) != hdr.run_len) {
            fprintf(stderr, "Error reading post-extract command.\n");
            fclose(f);
            return 1;
        }
        run_after[hdr.run_len] = '\0';
    }

    uint64_t payload_offset = header_offset + hdr.header_size;

    // Parse command line arguments
    const char *target_dir = default_target;
    int mode_list = 0;
    int mode_test = 0;
    int opt_overwrite = (hdr.flags & SFX_FLAG_OVERWRITE) ? 1 : 0;
    int opt_quiet = (hdr.flags & SFX_FLAG_QUIET) ? 1 : 0;
    int opt_target_specified = 0;

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-h") == 0 || strcmp(argv[i], "--help") == 0) {
            print_help(argv[0], title, default_target);
            fclose(f);
            return 0;
        } else if (strcmp(argv[i], "-l") == 0 || strcmp(argv[i], "--list") == 0) {
            mode_list = 1;
        } else if (strcmp(argv[i], "-t") == 0 || strcmp(argv[i], "--test") == 0) {
            mode_test = 1;
        } else if (strcmp(argv[i], "-y") == 0 || strcmp(argv[i], "--overwrite") == 0) {
            opt_overwrite = 1;
        } else if (strcmp(argv[i], "-q") == 0 || strcmp(argv[i], "--quiet") == 0) {
            opt_quiet = 1;
        } else if ((strcmp(argv[i], "-d") == 0 || strcmp(argv[i], "--target") == 0 || strcmp(argv[i], "-o") == 0) && i + 1 < argc) {
            target_dir = argv[++i];
            opt_target_specified = 1;
        }
    }

    // Graphical desktop prompt only if double-clicked / spawned without arguments
    char gui_target_buf[1024] = {0};
    if (!mode_list && !mode_test && !opt_quiet && !opt_target_specified && !getenv("LINRAR_NO_GUI") && !isatty(0) && (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY"))) {
        // Try zenity if present
        if (system("which zenity >/dev/null 2>&1") == 0) {
            char prompt_cmd[2048];
            snprintf(prompt_cmd, sizeof(prompt_cmd),
                "zenity --entry --title=\"LinRAR Self-Extractor\" --text=\"Extract '%s' to folder:\" --entry-text=\"%s\" 2>/dev/null",
                title[0] ? title : "Archive", target_dir);
            FILE *zp = popen(prompt_cmd, "r");
            if (zp) {
                if (fgets(gui_target_buf, sizeof(gui_target_buf) - 1, zp)) {
                    // Strip trailing newline
                    size_t gl = strlen(gui_target_buf);
                    while (gl > 0 && (gui_target_buf[gl - 1] == '\n' || gui_target_buf[gl - 1] == '\r')) {
                        gui_target_buf[--gl] = '\0';
                    }
                    if (gui_target_buf[0]) {
                        target_dir = gui_target_buf;
                    }
                }
                int zstatus = pclose(zp);
                if (WIFEXITED(zstatus) && WEXITSTATUS(zstatus) != 0) {
                    // User clicked Cancel in GUI dialog
                    fclose(f);
                    return 0;
                }
            }
        }
    }

    if (mode_list) {
        print_banner(title);
        printf("Listing archive contents:\n\n");
        char cmd[1024];
        if (hdr.payload_type == SFX_TYPE_ZIP) {
            snprintf(cmd, sizeof(cmd), "unzip -l -");
        } else {
            snprintf(cmd, sizeof(cmd), "tar -ztvf -");
        }
        int res = stream_payload_to_cmd(f, payload_offset, hdr.payload_len, cmd);
        fclose(f);
        return res;
    }

    if (mode_test) {
        print_banner(title);
        printf("Testing archive integrity...\n");
        char cmd[1024];
        if (hdr.payload_type == SFX_TYPE_ZIP) {
            snprintf(cmd, sizeof(cmd), "unzip -t -q -");
        } else {
            snprintf(cmd, sizeof(cmd), "tar -ztf - >/dev/null 2>&1");
        }
        int res = stream_payload_to_cmd(f, payload_offset, hdr.payload_len, cmd);
        if (res == 0) {
            printf("Integrity test PASSED: All files in SFX archive are valid.\n");
        } else {
            fprintf(stderr, "Integrity test FAILED: Archive payload contains errors or corrupted blocks.\n");
        }
        fclose(f);
        return res;
    }

    // Extraction Mode
    if (!opt_quiet) {
        print_banner(title);
        printf("Extracting archive contents to: %s\n", target_dir);
    }

    // Create target directory if needed
    char mkdir_cmd[2048];
    snprintf(mkdir_cmd, sizeof(mkdir_cmd), "mkdir -p \"%s\"", target_dir);
    if (system(mkdir_cmd) != 0) {
        fprintf(stderr, "Error: Could not create destination directory '%s'\n", target_dir);
        fclose(f);
        return 1;
    }

    char extract_cmd[4096];
    if (hdr.payload_type == SFX_TYPE_ZIP) {
        snprintf(extract_cmd, sizeof(extract_cmd),
            "unzip %s %s -d \"%s\" -",
            opt_overwrite ? "-o" : "",
            opt_quiet ? "-q" : "",
            target_dir);
    } else {
        snprintf(extract_cmd, sizeof(extract_cmd),
            "tar %s%s -C \"%s\" -f -",
            opt_quiet ? "-zx" : "-zxv",
            opt_overwrite ? " --overwrite" : "",
            target_dir);
    }

    int res = stream_payload_to_cmd(f, payload_offset, hdr.payload_len, extract_cmd);
    fclose(f);

    if (res != 0) {
        fprintf(stderr, "Error: Extraction encountered errors (exit code %d).\n", res);
        return res;
    }

    if (!opt_quiet) {
        printf("\nExtraction completed successfully.\n");
    }

    // Execute run_after command if defined
    if (run_after[0] != '\0') {
        if (!opt_quiet) {
            printf("Executing post-extraction command: %s\n", run_after);
        }
        char post_cmd[4096];
        snprintf(post_cmd, sizeof(post_cmd), "cd \"%s\" && %s", target_dir, run_after);
        int post_res = system(post_cmd);
        if (post_res != 0) {
            fprintf(stderr, "Warning: Post-extraction command exited with code %d\n", post_res);
        }
    }

    return 0;
}
