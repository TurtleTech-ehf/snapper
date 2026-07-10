/* Gate A: LoadLibrary + hs_init + snapper_pandoc_parse smoke (real C ABI). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

typedef void (*HsInitFn)(int *argc, char ***argv);
typedef void (*ReadyFn)(void);
typedef char *(*ParseFn)(const char *format, const char *input, char **err_out);
typedef void (*FreeFn)(char *ptr);

int main(int argc, char **argv) {
  const char *path = argc > 1 ? argv[1] : "snapper_pandoc.dll";
  HMODULE h = LoadLibraryA(path);
  if (!h) {
    fprintf(stderr, "LoadLibrary failed path=%s err=%lu\n", path,
            (unsigned long)GetLastError());
    return 2;
  }
  HsInitFn hs_init = (HsInitFn)GetProcAddress(h, "hs_init");
  ReadyFn ready = (ReadyFn)GetProcAddress(h, "snapper_pandoc_hs_ready");
  ParseFn parse = (ParseFn)GetProcAddress(h, "snapper_pandoc_parse");
  FreeFn freef = (FreeFn)GetProcAddress(h, "snapper_pandoc_free");
  if (!parse || !freef) {
    fprintf(stderr, "missing parse/free\n");
    return 1;
  }
  if (hs_init) {
    static char *arg0 = "snapper";
    static char *av[1];
    av[0] = arg0;
    int ac = 1;
    char **avp = av;
    hs_init(&ac, &avp);
  }
  if (ready)
    ready();
  char *err = NULL;
  char *json = parse("markdown", "# Title\n\nHello world.\n", &err);
  if (!json) {
    fprintf(stderr, "parse failed: %s\n", err ? err : "(null)");
    if (err && freef)
      freef(err);
    return 3;
  }
  /* Expect pandoc JSON with Title somewhere. */
  if (strstr(json, "Title") == NULL) {
    fprintf(stderr, "json missing Title: %.200s\n", json);
    freef(json);
    return 4;
  }
  printf("chk_parse OK (%zu bytes)\n", strlen(json));
  freef(json);
  FreeLibrary(h);
  return 0;
}
