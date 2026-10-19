import { applyD1Migrations } from "cloudflare:test";
import { env } from "cloudflare:workers";
import type { D1Migration } from "@cloudflare/vitest-plugin";
import type { Env } from "../src/env.ts";

// the schema of the database, applied before every test file (the storage of each test is isolated)
const e = env as unknown as Env & { TEST_MIGRATIONS: D1Migration[] };
await applyD1Migrations(e.DB, e.TEST_MIGRATIONS);
