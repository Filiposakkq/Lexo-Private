import { cloudflareTest, readD1Migrations } from "@cloudflare/vitest-plugin";
import { generateKeyPairSync } from "node:crypto";
import path from "node:path";
import { defineConfig } from "vitest/config";

// The tests run the Worker in the Workers runtime (Miniflare) with a real D1 database and a throw-away key pair, so no secret is ever needed.
export default defineConfig(async () => {
  const migrations = await readD1Migrations(path.join(import.meta.dirname, "migrations"));
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const seed = privateKey.export({ type: "pkcs8", format: "der" }).subarray(-32).toString("hex");
  const pub = publicKey.export({ type: "spki", format: "der" }).subarray(-32).toString("hex");
  return {
    plugins: [
      cloudflareTest({
        main: "./src/index.ts",
        wrangler: { configPath: "./wrangler.toml" },
        miniflare: {
          bindings: {
            LICENCE_SIGNING_KEY: seed,
            LICENCE_PUBLIC_KEY: pub,
            WEBHOOK_SECRET: "test-webhook-secret",
            ADMIN_TOKEN: "test-admin-token-0123456789",
            TEST_MIGRATIONS: migrations,
          },
        },
      }),
    ],
    test: { setupFiles: ["./test/apply-migrations.ts"] },
  };
});
