// The type of the main module for `exports.default` of `cloudflare:workers` in the tests.
declare namespace Cloudflare {
  interface GlobalProps {
    mainModule: typeof import("../src/index.ts");
  }
}
