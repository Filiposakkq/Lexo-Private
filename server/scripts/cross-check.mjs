// Checks that a licence file made by the server (WebCrypto Ed25519) is accepted by the Rust verifier of the Pro pack (`lexo-pro-licence show`).
// Usage: node scripts/cross-check.mjs <path to lexo-pro-licence.exe>   (makes its own throw-away key pair with the same tool)
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { signFile } from "../src/crypto.ts";
import { payloadFor } from "../src/licence.ts";

const tool = process.argv[2];
if (!tool) throw new Error("usage: node scripts/cross-check.mjs <lexo-pro-licence>");
const dir = mkdtempSync(join(tmpdir(), "aside-cross-"));
try {
  const out = execFileSync(tool, ["keygen", dir], { encoding: "utf8" });
  const publicKey = /public key:\s+([0-9a-f]{64})/.exec(out)?.[1];
  const seed = readFileSync(join(dir, "licence-private.key"), "utf8").trim();
  const row = { id: "L-CROSS01", features: "all", major: 1, check_days: 14 };
  const file = await signFile(seed, payloadFor(row, "any", Math.floor(Date.now() / 1000)));
  const path = join(dir, "licence.json");
  writeFileSync(path, JSON.stringify(file));
  const shown = JSON.parse(execFileSync(tool, ["show", path, publicKey], { encoding: "utf8" }));
  if (shown.state !== "licensed" || shown.id !== "L-CROSS01") throw new Error("the Rust verifier did not accept the file: " + JSON.stringify(shown));
  console.log("ok: the Rust verifier accepts a file signed by the server:", JSON.stringify(shown));
} finally {
  rmSync(dir, { recursive: true, force: true });
}
