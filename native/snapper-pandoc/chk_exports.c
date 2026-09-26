/* Gate A helper: resolve C ABI exports via GetProcAddress (same as libloading). */
#include <stdio.h>
#include <windows.h>

int main(int argc, char **argv) {
  const char *path = argc > 1 ? argv[1] : "snapper_pandoc.dll";
  HMODULE h = LoadLibraryA(path);
  if (!h) {
    fprintf(stderr, "LoadLibrary failed path=%s err=%lu\n", path,
            (unsigned long)GetLastError());
    return 2;
  }
  void *p = (void *)GetProcAddress(h, "snapper_pandoc_parse");
  void *f = (void *)GetProcAddress(h, "snapper_pandoc_free");
  void *r = (void *)GetProcAddress(h, "snapper_pandoc_hs_ready");
  void *i = (void *)GetProcAddress(h, "hs_init");
  printf("parse=%p free=%p ready=%p hs_init=%p\n", p, f, r, i);
  FreeLibrary(h);
  if (!p || !f) {
    fprintf(stderr, "missing required exports\n");
    return 1;
  }
  return 0;
}
