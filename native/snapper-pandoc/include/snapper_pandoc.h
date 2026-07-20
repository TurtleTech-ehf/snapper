/* C ABI for in-process pandoc parse used by snapper (Haskell FFI). */
#ifndef SNAPPER_PANDOC_H
#define SNAPPER_PANDOC_H

#ifdef __cplusplus
extern "C" {
#endif

/* Parse input as `format` (e.g. "markdown", "org") into a pandoc JSON AST.
 * On success returns a malloc'd UTF-8 C string owned by the caller; free with
 * snapper_pandoc_free. On failure returns NULL and, if err_out is non-NULL,
 * sets *err_out to a malloc'd error message (also freed with snapper_pandoc_free).
 */
char *snapper_pandoc_parse(const char *format, const char *input, char **err_out);

/* Free a string returned by snapper_pandoc_parse or an err_out message. */
void snapper_pandoc_free(char *ptr);

/* Optional explicit RTS touch (hs_init still required from the host). */
void snapper_pandoc_hs_ready(void);

#ifdef __cplusplus
}
#endif

#endif /* SNAPPER_PANDOC_H */
