/**
 * Rustype Core C API Header
 * Pure Vietnamese Telex composition engine.
 * https://namtr.dev
 */

#ifndef RUSTYPE_H
#define RUSTYPE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct RustypeTelex RustypeTelex;

/**
 * Creates a new Telex composition state.
 * @param modern_orthography false: hòa, khỏe, thúy. true: hoà, khoẻ, thuý.
 * @param auto_restore_english Restore English words on word boundary.
 */
RustypeTelex* rustype_telex_new(bool modern_orthography, bool auto_restore_english);

/**
 * Frees a Telex instance.
 */
void rustype_telex_free(RustypeTelex* telex);

/**
 * Resets the composition state (e.g. on cursor move, focus change).
 */
void rustype_telex_reset(RustypeTelex* telex);

/**
 * Processes a single character key press.
 * @param ch Unicode codepoint.
 * @param out_handled Set to true if the key was handled by the engine.
 * @param out_backspaces Number of characters to delete before the cursor.
 * @param out_commit Buffer to receive the UTF-8 text to insert.
 * @param out_commit_capacity Size of out_commit buffer.
 * @return true if handled, false if passed to the app.
 */
bool rustype_telex_key(
    RustypeTelex* telex,
    uint32_t ch,
    bool* out_handled,
    size_t* out_backspaces,
    char* out_commit,
    size_t out_commit_capacity
);

/**
 * Processes a backspace key.
 * @return true if handled, false if passed to the app.
 */
bool rustype_telex_backspace(
    RustypeTelex* telex,
    bool* out_handled,
    size_t* out_backspaces,
    char* out_commit,
    size_t out_commit_capacity
);

/**
 * Feeds a string of keys and writes the composed output to out_buf.
 * @return Number of bytes written (excluding terminating null byte).
 */
size_t rustype_telex_feed(
    RustypeTelex* telex,
    const char* input,
    char* out_buf,
    size_t out_buf_len
);

/**
 * Returns the version string of rustype-core.
 */
const char* rustype_version(void);

#ifdef __cplusplus
}
#endif

#endif /* RUSTYPE_H */
