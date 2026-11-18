import assert from "node:assert/strict";
import test from "node:test";
import {
  applyFormatOptions,
  normalizeDetectedFormat,
} from "../src/options.js";
import { Format, type FormatOptions } from "../src/types.js";

type Assert<T extends true> = T;
type HasKey<T, K extends PropertyKey> = K extends keyof T ? true : false;
type _path = Assert<HasKey<FormatOptions, "path">>;
type _ignore = Assert<HasKey<FormatOptions, "ignore">>;
type _nullable = Assert<
  null extends ReturnType<typeof normalizeDetectedFormat> ? true : false
>;
const _publishedContract: [_path, _ignore, _nullable] = [true, true, true];
void _publishedContract;

test("Format includes Typst and AsciiDoc at the wasm enum values", () => {
  assert.equal(Format.Org, 0);
  assert.equal(Format.Latex, 1);
  assert.equal(Format.Markdown, 2);
  assert.equal(Format.Rst, 3);
  assert.equal(Format.Plaintext, 4);
  assert.equal(Format.Typst, 5);
  assert.equal(Format.AsciiDoc, 6);
});

test("applyFormatOptions forwards path and ignore", () => {
  const calls: string[] = [];
  applyFormatOptions(
    {
      set_format() {},
      set_max_width() {},
      set_lang() {},
      add_abbreviation() {},
      set_path(path: string) {
        calls.push(`path:${path}`);
      },
      add_ignore(pattern: string) {
        calls.push(`ignore:${pattern}`);
      },
    },
    { path: "docs/a.md", ignore: ["skip/**", "vendor/**"] },
  );
  assert.deepEqual(calls, [
    "path:docs/a.md",
    "ignore:skip/**",
    "ignore:vendor/**",
  ]);
});

test("applyFormatOptions leaves path and ignore alone when they are omitted", () => {
  const calls: string[] = [];
  applyFormatOptions(
    {
      set_format() {},
      set_max_width() {},
      set_lang() {},
      add_abbreviation() {},
      set_path(path: string) {
        calls.push(`path:${path}`);
      },
      add_ignore(pattern: string) {
        calls.push(`ignore:${pattern}`);
      },
    },
    { maxWidth: 72 },
  );
  assert.deepEqual(calls, []);
});

test("normalizeDetectedFormat is null when the extension is refused", () => {
  assert.equal(normalizeDetectedFormat(undefined), null);
  assert.equal(normalizeDetectedFormat(null), null);
  assert.equal(normalizeDetectedFormat(Format.Plaintext), Format.Plaintext);
});
