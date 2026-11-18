declare module "node:test" {
  function test(name: string, fn: () => void): void;
  export default test;
}

declare module "node:assert/strict" {
  export function deepEqual(
    actual: unknown,
    expected: unknown,
    message?: string,
  ): void;
  export function equal(
    actual: unknown,
    expected: unknown,
    message?: string,
  ): void;
}
