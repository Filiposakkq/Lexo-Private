import { cloudflareTest, readD1Migrations } from "@cloudflare/vitest-plugin";
import path from "node:path";
import { defineConfig } from "vitest/config";

// The tests run the Worker in the Workers runtime (Miniflare) with a real D1 database; no secret and no network are needed
// (without RESEND_API_KEY nothing is mailed, without TURNSTILE_SECRET the forms are not checked).
export default defineConfig(async () => {
  const migrations = await readD1Migrations(path.join(import.meta.dirname, "migrations"));
  return {
    plugins: [
      cloudflareTest({
        main: "./src/index.ts",
        wrangler: { configPath: "./wrangler.toml" },
        miniflare: { bindings: { ADMIN_TOKEN: "test-admin-token-0123456789", TEST_MIGRATIONS: migrations } },
      }),
    ],
    test: { setupFiles: ["./test/apply-migrations.ts"] },
  };
});
