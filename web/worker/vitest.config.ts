import { cloudflareTest, readD1Migrations } from "@cloudflare/vitest-plugin";
import { generateKeyPairSync } from "node:crypto";
import path from "node:path";
import { defineConfig } from "vitest/config";

// The tests run the Worker in the Workers runtime (Miniflare) with a real D1 database; no secret and no network are needed
// (without RESEND_API_KEY nothing is mailed, without TURNSTILE_SECRET the forms are not checked).
export default defineConfig(async () => {
  const migrations = await readD1Migrations(path.join(import.meta.dirname, "migrations"));
  // a throw-away RSA key in the place of Cloudflare Access: the tests sign their own Access tokens with it
  const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
  const pub = { ...publicKey.export({ format: "jwk" }), kid: "test-key", alg: "RS256" };
  return {
    plugins: [
      cloudflareTest({
        main: "./src/index.ts",
        wrangler: { configPath: "./wrangler.toml" },
        miniflare: {
          bindings: {
            ADMIN_TOKEN: "test-admin-token-0123456789",
            HASH_SALT: "test-salt",
            ACCESS_TEAM_DOMAIN: "aside-test.cloudflareaccess.com",
            ACCESS_AUD: "test-aud-0123",
            ACCESS_JWKS: JSON.stringify({ keys: [pub] }),
            TEST_ACCESS_KEY: JSON.stringify(privateKey.export({ format: "jwk" })),
            TEST_MIGRATIONS: migrations,
          },
        },
      }),
    ],
    test: { setupFiles: ["./test/apply-migrations.ts"] },
  };
});
